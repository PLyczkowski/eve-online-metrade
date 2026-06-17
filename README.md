# EVE Metrade

Local desktop app for finding EVE Online hauling opportunities between Jita and Amarr.

## Run Web UI

```powershell
npm install
npm run dev
```

## Run Tests

```powershell
npm test
```

## Run Tauri App

Install Rust first, then run:

```powershell
npm run tauri dev
```

The frontend works in a browser with local fallback storage. The Tauri build uses the native backend and SQLite.
