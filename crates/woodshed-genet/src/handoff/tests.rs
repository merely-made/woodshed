use super::*;

#[test]
fn owner_lock_excludes_a_second_writer_and_recovers_a_stale_address() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join(ENDPOINT), b"stale").unwrap();
    let owner = acquire(root)
        .unwrap()
        .expect("stale address cannot own the root");
    assert!(acquire(root).unwrap().is_none());
    drop(owner);
    assert!(!root.join(ENDPOINT).exists());
    assert!(acquire(root).unwrap().is_some());
}

#[test]
fn handoff_protocol_accepts_exact_actions_and_bounds_each_frame() {
    let mut frames = &b"{\"token\":\"test\",\"command\":{\"action\":\"status\"}}\n{\"token\":\"test\",\"command\":{\"action\":\"detach\"}}\n"[..];
    assert!(matches!(
        read_frame::<Request>(&mut frames).unwrap().unwrap().command,
        Command::Status
    ));
    assert!(matches!(
        read_frame::<Request>(&mut frames).unwrap().unwrap().command,
        Command::Detach
    ));
    assert!(read_frame::<Request>(&mut frames).unwrap().is_none());
    assert!(read_frame::<Request>(&mut &b"{}"[..]).is_err());
    assert!(read_frame::<Request>(&mut vec![b'x'; MAX_FRAME as usize + 1].as_slice()).is_err());
}

#[test]
fn authenticated_pane_survives_idle_and_disconnect_releases_ownership() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, packets) = mpsc::sync_channel(1);
    let (wake, wakeups) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (connection, _) = listener.accept().unwrap();
        run_peer(connection, 1, "test", &sender, &|| wake.send(()).unwrap())
    });
    let mut client = BufReader::new(TcpStream::connect(address).unwrap());
    client
        .get_ref()
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut dispatch = |command| {
        write_frame(
            client.get_mut(),
            &Request {
                token: "test".into(),
                command,
            },
        )
        .unwrap();
        let packet = packets.recv_timeout(Duration::from_secs(2)).unwrap();
        let result = packet.alive.clone();
        packet.response.send(Ok(json!({}))).unwrap();
        assert!(read_frame::<Response>(&mut client).unwrap().unwrap().ok);
        wakeups.recv_timeout(Duration::from_secs(2)).unwrap();
        result
    };
    let lease_alive = dispatch(Command::Attach);
    std::thread::sleep(Duration::from_millis(5_250));
    assert!(lease_alive.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(&lease_alive, &dispatch(Command::Play)));
    drop(client);
    wakeups.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!lease_alive.load(Ordering::Acquire));
    worker.join().unwrap().unwrap();
}

#[test]
fn a_detached_lease_allows_a_new_import_without_granting_new_or_stale_controls() {
    use woodshed_core::working_sets::WorkingSetId;

    let mut lease = Lease {
        peer: 1,
        alive: Arc::new(AtomicBool::new(true)),
        session: Arc::new(()),
        owner: WorkingSetId(1),
        attached: true,
    };
    let import = Command::ImportWav {
        source: WavPhraseSource {
            title: "Second recording".into(),
            source_page_url: "file:///second.wav".into(),
            start_ms: 0,
            end_ms: 100,
            note: String::new(),
        },
        wav_path: PathBuf::from("second.wav"),
        preview_path: None,
    };
    let page_import: Command = serde_json::from_value(json!({
        "action": "import-page-phrase",
        "source": {
            "title": "Page take", "source_page_url": "https://example.test/lesson",
            "start_ms": 200, "end_ms": 300, "note": "Repeat"
        },
        "phrase_path": "page.wav"
    }))
    .unwrap();

    assert!(!lease.permits(2, &import));
    assert!(!lease.permits(2, &page_import));
    lease.attached = false;
    assert!(lease.permits(2, &import));
    assert!(lease.permits(2, &page_import));
    assert!(!lease.permits(2, &Command::Attach));
    assert!(!lease.permits(2, &Command::Play));
    assert!(lease.permits(1, &Command::Play));

    // After a successful import, the new peer owns the new recording.
    lease.peer = 2;
    lease.owner = WorkingSetId(2);
    assert!(lease.permits(2, &Command::Attach));
    assert!(lease.permits(2, &Command::Play));
    assert!(!lease.permits(1, &Command::Attach));
    assert!(!lease.permits(1, &Command::Pause));
}
