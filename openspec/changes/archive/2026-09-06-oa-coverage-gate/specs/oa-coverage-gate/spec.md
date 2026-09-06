## ADDED Requirements
### Requirement: Coverage enforcement
CI SHALL enforce a minimum test coverage.
#### Scenario: Coverage below floor
- **WHEN** coverage drops below the floor
- **THEN** CI fails
#### Scenario: Coverage ok
- **WHEN** coverage meets the floor
- **THEN** CI passes and reports the value
