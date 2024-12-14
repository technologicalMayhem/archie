# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Config option for update check interval
- Users can request rebuilds manually

### Fixed

- Database was not created if no packages were being tracked
- Old database file not being deleted if the name of the repository is changed
- Coordinator not shutting down if any part of it shuts down unexpectedly

## [0.2.0] - 2024-12-12

### Added

- Discovery of dependencies
- More info to the status command, including server info, warnings, and the list of packages
- Multiple configuration profiles for the client
- Package remove functionality
- Apps print their version

### Changed

- Set the worker container's name to the package name
- Default number of builders changed to 1
- Changed error levels to warnings or debug for better clarity
- Refactored internal communication mechanisms for better maintainability

### Fixed

- Fixed non-built packages not being built upon restart
- Fixed orchestrator reporting forcefully stopped containers as exited abnormally
- Fixed repository trying to remove never-built packages
- Fixed scheduler bugs:
    - Prevent unnecessary waiting during the first loop
    - React to new messages immediately
- Fixed worker bugs:
    - Corrected endpoint configuration
    - Improved log levels, moving less critical items to debug
- Fixed repository bugs:
    - Prevent running commands when no repository exists
    - Corrected error reporting for repository operations
- Fixed max retries not being respected
- Fixed bugs with CLI and webserver endpoints:
    - Corrected struct usage in remove command
    - Fixed swapped webserver endpoints

### Removed

- Removed unneeded dependencies
- Removed unneeded log statements
- Removed unused repository lock field
- Removed unneeded imports
- Removed accidental println statements

## [0.1.0] - 2024-12-06

Initial implementation of the application

- Core functionality to build and rebuild packages as necessary when updates are pushed
- CLI application for sending build requests
- Configuration management through environment variables
- Graceful termination of the application using signal handling

[Unreleased]: https://git.techmayhem.net/techMayhem/archie/compare/v0.2.0...HEAD
[0.2.0]: https://git.techmayhem.net/techMayhem/archie/compare/v0.1.0...v0.2.0
[0.1.0]: https://git.techmayhem.net/techMayhem/archie/releases/tag/v0.1.0