# Security Policy

## Supported versions

The project is young and has no release yet: the `main` branch is the supported one.

| Version | Supported |
| ------- | --------- |
| `main`  | ✅ |

## What counts as a security issue here

Manto.RustPad opens files it does not control. Reports that matter most:

- a crash, hang or out-of-bounds read triggered by a crafted file;
- any case where the program **writes something other than what the user asked**
  (the editing layer is not implemented yet, but this will be the rule once it is);
- unbounded memory use that depends on the content of a file.

## Reporting a vulnerability

Use GitHub's **private vulnerability reporting**: the *Report a
vulnerability* button on the Security tab reaches the maintainer without
public exposure. Please do not open a public issue for security matters.
