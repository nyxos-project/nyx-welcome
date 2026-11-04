# Nyx Welcome

NyxOS's setup after installation. It opens at the first sign-in, and is in the launcher
afterwards to change anything again. It's separate from the installer.

The steps, each but the first and last skippable:

1. Welcome
2. Appearance: the theme (for the lock screen too, and optionally the boot and sign-in
   screens), and density
3. Computer and desktop: name, time zone, keyboard, screens, and the desktop's pages in
   Settings
4. Starting point: the Everyday, Developer, AI Developer and Homelab presets, combinable
5. Nyx apps, Apps, Command-line tools, Development, Containers and infrastructure, AI
   agents: the NyxOS software catalog's sections
6. Default apps
7. Accounts and sign-in: your account, online accounts, face and fingerprint
8. Updates, backup and recovery
9. Review and apply

Theme, default apps and accounts change right away, for you. System settings and
software for everyone wait for Review, which applies them in one rebuild through the
Nyx System Manager (built first, switched to only when it works, undone otherwise), then
installs Flathub apps for you and updates NyxOS if asked. Unfree licenses, services and
groups an item needs are listed there, and each has to be agreed to. A failed step can
be tried again; what already worked isn't repeated.

The software is the catalog Nyx Store uses (`nyx-catalog`, in
[nyx-system](https://github.com/nyxos-project/nyx-system)).

```sh
nyx-welcome                 # open it
nyx-welcome --step agents   # at a step
nyx-welcome --first-run     # open only while setup hasn't been finished
```

`~/.config/nyx/welcome.toml` records whether setup was finished and the steps skipped.
`NYX_WELCOME_APP_ID` runs a test copy under its own app ID.

## Build

```sh
cargo build --release
cargo test --workspace
```

## License

MIT
