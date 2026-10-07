# Security policy

Security fixes target the current default branch and latest release, when one
exists. Older versions are not maintained separately.

Please use [GitHub private vulnerability reporting](https://github.com/joshmcadams/heads/security/advisories/new)
to report vulnerabilities. Include the heads and Git versions, platform,
reproduction steps, and potential impact. Remove credentials and private data.
If private reporting is unavailable, contact the maintainer through the contact
information on [their GitHub profile](https://github.com/joshmcadams) to arrange
a private report. Avoid putting exploit details in a public issue.

heads invokes system Git and honors its hooks, helpers, and configuration.
Only run it over repositories you trust. See the README's
[safety notes](README.md#safety) for branch changes and interruption behavior.
