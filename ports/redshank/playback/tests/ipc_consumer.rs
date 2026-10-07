//! Consumer checks for Genet's selected in-process IPC fork, through its
//! public media-player reexport. These do not qualify cross-process IPC.

use servo_media_player::ipc_channel::{
    IpcError, TryRecvError,
    ipc::{self, IpcSender},
};
use std::time::Duration;

type Payload = (u64, String, Vec<u8>);
const DEADLINE: Duration = Duration::from_secs(1);

#[test]
fn typed_messages_preserve_text_bytes_and_order() {
    let (sender, receiver) = ipc::channel::<Payload>().unwrap();
    let first = (41, "episode:β".into(), vec![0, 127, 255]);
    let second = (42, "note:two".into(), Vec::new());
    sender.send(first.clone()).unwrap();
    sender.send(second.clone()).unwrap();

    assert_eq!(receiver.try_recv_timeout(DEADLINE).unwrap(), first);
    assert_eq!(receiver.try_recv_timeout(DEADLINE).unwrap(), second);
}

#[test]
fn transferred_sender_retains_the_original_receiver() {
    let (sender, receiver) = ipc::channel::<Payload>().unwrap();
    let (transfer, transferred) = ipc::channel::<IpcSender<Payload>>().unwrap();
    transfer.send(sender).unwrap();
    let sender = transferred.try_recv_timeout(DEADLINE).unwrap();
    drop(transfer);
    drop(transferred);
    let payload = (7, "reply".into(), vec![3, 2, 1]);
    sender.send(payload.clone()).unwrap();

    assert_eq!(receiver.try_recv_timeout(DEADLINE).unwrap(), payload);
}

#[test]
fn disconnect_follows_the_last_sender_and_drains_queued_data() {
    let (sender, receiver) = ipc::channel::<Payload>().unwrap();
    let last_sender = sender.clone();
    let payload = (9, "last".into(), vec![9]);
    sender.send(payload.clone()).unwrap();
    drop(sender);
    assert_eq!(receiver.try_recv_timeout(DEADLINE).unwrap(), payload);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    drop(last_sender);

    assert!(matches!(
        receiver.try_recv_timeout(DEADLINE),
        Err(TryRecvError::IpcError(IpcError::Disconnected))
    ));
}
