# Self-hosting liber

Run one copy of liber as the server and use phones and other desktops as
thin clients. One collection, one writer of record, no sync conflicts ever:
the journal and merge machinery simply have nothing to do in this topology.

This guide assumes a machine that is always on (desktop, home server, VPS).
If that does not describe you, the folder-sync methods in
[syncing-guide.md](syncing-guide.md) remain fully supported.

## Trust model

liber has no login. Access control is entirely the transport's job, so the
transport is not optional:

- **Never** expose `liber --serve` directly to the internet.
- Use **one** of: a TLS-terminating reverse proxy (Caddy, Nginx) with a real
  hostname, or a mesh VPN (Tailscale, WireGuard) where the port never leaves
  your private network. The VPN path needs no TLS certificates and is the
  recommended default.
- Token auth does not exist yet. Treat every URL that reaches the server as
  fully trusted, because it is.

## Server setup

1. Install liber on the host (binary, `go build`, nix package, or Arch
   package; see README install section).
2. Pick where the collection lives and point liber at it:
   ```sh
   liber config set base_dir ~/Bookmarks
   ```
3. Run the server bound to the LAN (or to loopback behind a local reverse
   proxy on the same machine):
   ```sh
   liber --serve --addr 0.0.0.0:8080     # LAN / VPN interface
   liber --serve                         # loopback only (default)
   ```
   Binding anything but loopback prints a warning. That warning is the
   trust model reminding you it exists.
4. Keep it running across reboots. Example systemd user unit
   (`~/.config/systemd/user/liber.service`):
   ```ini
   [Unit]
   Description=liber bookmark server
   After=network-online.target
   Wants=network-online.target

   [Service]
   ExecStart=/usr/local/bin/liber --serve --addr 127.0.0.1:8080
   Restart=on-failure

   [Install]
   WantedBy=default.target
   ```
   ```sh
   systemctl --user enable --now liber
   ```

### Reverse proxy (Caddy)

```caddy
bookmarks.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Caddy fetches and renews the certificate automatically. Nginx equivalent is
a standard `proxy_pass` block plus certbot; any working TLS proxy is fine,
liber does not care what terminates TLS.

### Tailscale / WireGuard

Bind liber to loopback or the tailnet IP and reach it over the mesh. No
certificates, no open ports, no DNS. This is the simplest correct setup.

## Backups

The collection is flat files, so back up the directory, not a database dump:

```sh
restic -r /backup/liber backup ~/Bookmarks
# or: rsync -a ~/Bookmarks/ /backup/liber/
```

If `base_dir` is inside a git repo, `liber --sync` (optionally `-p`) commits
snapshots on top of file backups. Exclude `site/`, `.liber/resolved/`,
`.liber/restage/`, and `*.tmp` from sync-style backups; see
[syncing-guide.md](syncing-guide.md) for the full exclusion list.

## Connecting clients

- **Desktop browser:** open the server URL. Everything works, including
  full-page archives rendered with the server's backends.
- **Android app:** choose server mode on first launch (or later from the
  menu), enter the server URL, and use only URLs you trust (tunnel or local
  network). Uploads, downloads, share intents, and history all work against
  the remote origin; exports run on the server and are downloaded from its
  settings page.
- **Bookmarklet:** same snippet as local use, pointed at the server:
  ```
  javascript:location.href='https://bookmarks.example.com/?prefill='+encodeURIComponent(location.href)
  ```

## Operational notes

- Concurrent clients are within the design (`writeMu` + per-request fresh
  loads). Two browsers editing at once is safe at this scale.
- Updates: stop the server, replace the binary, start it. The JSON index and
  journal are backward compatible; old clients ignore unknown journal fields.
- If the server ever moves or the URL changes, clients just point at the new
  address. No per-client migration exists because clients hold no state.
- Port conflicts: pass `--addr` with a free port; the Android wrapper picks
  one automatically for its embedded server.
