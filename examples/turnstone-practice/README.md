# Practice a phrase from Turnstone in Woodshed

## Mark and loop four bars of piano

“Window Seat” is an original six-bar stereo piano take in 4/4 at 120 BPM.
Play the take in the Turnstone practice page. Choose a bar to seek to it, then use **Mark in** and **Mark out** while listening. Each mark snaps to the start of the bar you hear. Mark bar 2 as the start and bar 6 as the end to repeat bars 2 through 5. Add a note and choose **Practice this phrase**.

Woodshed stores the WAV with your practice and starts looping the selected bars. Choose **Attach** in the transport pane to move the controls into Turnstone, then **Detach** to bring back the Woodshed window.

From the Woodshed checkout on Windows, run the following for an interactive session. Open the printed local address in Turnstone's **Genet (Bole)** viewer. The preview in this viewer uses the Windows WAV player, which supplies the position used for each mark.

```powershell
python -B examples/turnstone-practice/piano_practice.py --manual --woodshed C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --keeper C:/t/cargo-targets/graphshell/debug/tucket-custody-receipt-host.exe --receipt C:/Users/mark_/Code/testing/woodshed/receipts/my-piano-phrase
```

The runner uses an isolated Woodshed store. Keep `phrase.json` and `practice-clip.wav` together. Open them in your own Woodshed practice from a terminal with your usual custody service running.

```powershell
C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --open-wav-phrase C:/Users/mark_/Code/testing/woodshed/receipts/my-piano-phrase/phrase.json
```

For the scripted check, replace `--manual` with `--turnstone <Bole-enabled-turnstone.exe> --turnstone-root <Turnstone-checkout>` and use a fresh receipt directory. The runner selects Bole with `TURNSTONE_GENET_ENGINE=bole` and checks the active engine in the native window. The result includes the original take, an audible excerpt (`marked-phrase.wav`), the marks and note, and the observed audio-device frames across attach, detach, and restart.

## Mark a passage in “Fired”

Start the local receiver from the Woodshed checkout and open the address it prints in Turnstone’s **Genet (Bole)** viewer. Use a fresh receipt directory each time.

```powershell
python -B examples/turnstone-practice/practice.py --mark-listening --woodshed C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --receipt C:/Users/mark_/Code/testing/woodshed/receipts/my-fired-phrase
```

Listen to the [official recording of “Fired”](https://www.youtube.com/watch?v=7_xQ4o796H0) and enter the in and out times in seconds on the Turnstone page. Add your practice note and choose **Practice this phrase**.

Woodshed saves the recording reference, both marks, and your note together. The receiver uses an isolated working Set and exports `fired.listening.json` to your receipt directory. To keep that phrase in your own Woodshed practice, open the exported file from the desktop command line.

```powershell
C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --open-listening-phrase C:/Users/mark_/Code/testing/woodshed/receipts/my-fired-phrase/fired.listening.json
```

In **Arrangement**, **Open original recording** opens YouTube at the start of the passage. The end mark is a practice cue. Media playback stays with the recording provider.

To run the desktop regression, use the command in the guitar section below with `--listening` and a fresh receipt directory. It enters 72.000 and 76.500 seconds and a note in Turnstone, imports them into Woodshed, closes the source, then checks the saved Set, the portable file import, and the reopened Arrangement view. Both modes accept the isolated test identity described below.

## Import a two-bar guitar study

The lesson pairs Led Zeppelin's [official recording of “Bron-Y-Aur Stomp”](https://www.youtube.com/watch?v=U2xFLLG2jqk) with an original alternating-thumb study. The study uses standard guitar tuning and two bars of even eighth notes in 4/4 at 80 bpm. The [published guitar tab](https://www.alfred.com/products/bron-y-aur-stomp-00-ps-0000262) is available separately.

Select the study in the lesson and choose **Use in Woodshed**. The receiver sends the selected text, the lesson address, the listening link, and the stated tuning and rhythm to Woodshed. Woodshed creates a working Set and saves its MusicXML and source selection together. The proof runs in an isolated Woodshed data directory.

On a Windows desktop with Woodshed's custody service available, use Python 3.11+, a Woodshed desktop build and Turnstone with the `scripted-webgl` feature. Choose a new receipt directory.

```powershell
python -B examples/turnstone-practice/practice.py --woodshed C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --turnstone C:/t/cargo-targets/turnstone/debug/turnstone.exe --turnstone-root C:/Users/mark_/Code/repos/turnstone --receipt C:/Users/mark_/Code/testing/woodshed/receipts/selected-guitar-study
```

The receiver closes the lesson and both applications, then reopens the same saved practice in a fresh Woodshed process. It checks each written note against the selected string and fret, compares the reopened score and source with the original import, and observes the audio device play through the loop. The receipt includes the exported `study.musicxml`, `selection.json`, and the run outcome.

For a synthetic identity, add `--keeper PATH/TO/tucket-custody-receipt-host.exe`. The receiver starts it in `woodshed-practice` mode and closes it after verification. Build that test executable from the Graphshell checkout with `cargo build --manifest-path host/Cargo.toml -p mere-resident --bin tucket-custody-receipt-host --features custody-receipt-host`.
