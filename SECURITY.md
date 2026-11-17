# Security

freedock runs with your user's rights, hides and restores the Windows taskbar, reads window titles
to list your apps, and can install an optional keyboard hook (only for the two Windows-key settings,
both off by default). It keeps your settings in `%APPDATA%\freedock`; calendar links and the MCP
server link are encrypted for your Windows user (DPAPI).

If you find a security problem, please don't open a public issue. Report it privately through
GitHub's **Security > Report a vulnerability** on this repository, with steps to reproduce. You'll
get a reply within a week.
