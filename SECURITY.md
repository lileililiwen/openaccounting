# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in OpenAccounting,
please report it responsibly. **Do NOT open a public GitHub
issue for security vulnerabilities.**

Email: lileililiwen@gmail.com

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

## Disclosure Process

1. Report by email (above). **Do NOT open a public issue.**
2. The maintainer acknowledges receipt within 48 hours.
3. Assessment within 1 week: severity, affected versions, workaround.
4. Fix or mitigation within 30 days for critical issues; coordinated
   disclosure date agreed with the reporter before any public note.
5. Credit in the release notes unless the reporter prefers anonymity.

The full process, threat model, and log-handling rules live in
`docs/threat-model.md`. Report handling follows `GOVERNANCE.md`
(triage SLA); vulnerability reports take priority over the standard
5-business-day queue.

## Response Timeline

- **Acknowledgment:** within 48 hours
- **Initial assessment:** within 1 week
- **Fix or mitigation:** within 30 days for critical issues

## Scope

In scope:
- Authentication and session management
- Authorization and access control
- SQL injection and data exposure
- Cryptographic weaknesses
- Dependency vulnerabilities with known exploits

Out of scope:
- Denial of service against the application itself
- Social engineering
- Issues requiring physical access to the server

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x | Yes |

## Security Best Practices for Deployment

- Set `APP_SECRET` to a random string ≥ 64 characters
- Set `APP_ENV=production` to enable strict cookie and config checks
- Use HTTPS in front of the application (reverse proxy)
- Restrict database access to the application network only
- Enable PostgreSQL SSL connections for remote databases
- Regularly update dependencies (`cargo audit`)
