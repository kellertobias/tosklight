# Set up audio

Open **Audio** to check the input Pixel uses for generated visuals and beat effects. The meters and kick, snare/clap and hi-hat lamps show what the engine hears.

![Pixel audio signal and instrument detection](../assets/screenshots/workflows/pixel-audio.png)

## Select and check an input

1. Choose **Change audio settings** in the title bar.
2. Select **Input**, or choose **A device by name…** and enter its exact name.
3. Restart Pixel after changing the input device.
4. Play a representative signal through that input and check the meters and instrument lamps.
5. Adjust **Gain**, **Automatic gain**, **Beat sensitivity**, **Bass**, **Mid** and **Treble** as needed.
6. Choose **Done** to close the settings controls.

Settings save while you adjust them. Gain, automatic gain, beat sensitivity and EQ reach the running analysis immediately. Gain is a trim when **Automatic gain** is enabled. **Beat sensitivity** changes how readily Pixel detects quieter instrument hits, independently of Gain. A missing named device reports an error instead of falling back to the default.

If macOS has not decided whether Pixel Media Server may use the microphone, the **Audio** page shows **Request microphone access**. Approve the system prompt and Pixel opens the selected input immediately. If access was denied, enable Pixel Media Server in macOS **System Settings > Privacy & Security > Microphone**, then press **Check microphone access**. If permission is granted but no stream is open, press **Start audio input**.

A moving visualizer preview does not establish that it hears the intended input. Check the live meters and instrument lamps on the production computer. A still Triangular Net between songs is expected; a still net during music calls for checking those lamps.

## Route playback sound

Choose the output's sound device in **Settings > Sound** and restart Pixel to open that device. Rehearse the physical output and latency with the production interface. The system default may change when devices are connected or disconnected.

Layer **Volume** and Master Volume remain playback controls. Selecting an analysis input does not replace the sound-output setting. For synchronized clips, [choose Playback BPM or a Control Speed Group](09-connect-the-desk.md#follow-the-desk-s-speed-groups).
