# Privacy

* **Local only.** All data lives in one SQLite file in the app's data folder
  (Windows: `%APPDATA%\org.opendiag.app\opendiag.db`; Android: the app's private storage).
* **No account, no telemetry, no automatic upload.** Not the VIN, not reports, not location.
* **Update check.** The installed app asks GitHub's public API which OpenDiag
  release is the newest when it starts (`update.rs` in the app shell). The
  request contains nothing about the user or vehicle (GitHub sees the IP address,
  as with any web request). Settings → Updates turns it off. Updates are only
  downloaded when the user presses the button.
  Community features (submitting compatibility results) work by exporting a
  file the user chooses to share. Any future online service will be opt-in.
* **Logs.** The adapter log hides VIN answers unless the user turns that off.
* **Exports** show what they contain before saving and mask the VIN unless the
  user ticks "Include the full VIN".
* **Deletion.** Settings → Privacy deletes every vehicle, session, report, log
  and clear snapshot. Deleting a vehicle in My Garage deletes its history.
* **At-rest encryption** is not implemented yet. Until it is, rely on device
  encryption (BitLocker, Android's default file-based encryption). Planned:
  encrypt the database with a key kept in the OS keystore (Windows DPAPI /
  Android Keystore).
