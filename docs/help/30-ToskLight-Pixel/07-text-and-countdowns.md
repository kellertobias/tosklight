# Show text, clocks and countdowns

Text content occupies folders `200`-`249`. Open **Text**, select a slot and configure its words and presentation. Recall its numeric folder/file address on a Media layer. Keep the chosen fonts available on the production server.

## Set venue time

Clocks and clock-derived text use Pixel's UTC offset. Open **Settings > Libraries > Server time** and enter minutes east of UTC using the number field, step buttons or touch keypad. Values outside ±840 minutes are refused; the section reports the save result. The offset is stored configuration and applies to the next drawn frame, so a clock on an output follows an accepted change without a restart. The host computer's timezone does not change this offset. A single clock that has to show another city can carry **Own UTC offset**; every other clock keeps following the server.

## Choose a countdown

- **Countdown of a length** starts when its layer becomes visible.
- **Countdown to a moment** counts to one date and time.
- **Countdown to a time every day** counts to a local time, such as 21:00, without a date.

The daily countdown works before each show. It counts from midnight to that day's time. Once the time has passed, it does what **After zero** says: hold at zero, go on with a minus sign, or count up. At the next midnight it starts counting to the next day's time. "Local" and "midnight" follow the server's UTC offset, like a clock. That offset is fixed, so a daylight-saving change moves the target only when the offset in **Settings > Libraries > Server time** is changed.
