# Overview

**Archie** and the **Coordinator** are tools designed to simplify building and managing packages from the Arch User
Repository (AUR) for Arch Linux.

- **Archie** is a CLI tool that interacts with the Coordinator to request package builds, check statuses, and manage
  repository contents.
- The **Coordinator** is a server application that orchestrates the package building process, leveraging Docker
  containers to compile AUR packages in a controlled environment. It also automatically detects when packages update to
  ensure it serves the most up-to-date version of any package it manages.

Together they should make it easy to install and keep AUR packages up to date. With the Coordinator processing updates
in the background and providing a repository of packages, the experience for handling package updates should be similar
to the official repositories.

# Setup

## Dependencies

### For Archie (cli tool)

To compile the CLI tool you need git, OpenSSL and Rust. Ensure you have them by running:

```
sudo pacman -Sy --needed git openssl rust
```

### For Coordinator (server)

To run the coordinator and worker you need to have docker installed.

For Arch just run `sudo pacman -Sy --needed docker docker-compose`.
You might need to enable the docker daemon afterwards by running `sudo systemctl enable --now docker.service`.

For Debian, follow
the [official instructions](https://docs.docker.com/engine/install/debian/#install-using-the-repository).

## Installing Archie

To install the CLI tool, run:

```
cargo install --git https://git.techmayhem.net/techmayhem/archie\#alpha-2 --bin archie
```

## Setting up Coordinator and Worker

Pull the image for the coordinator and the worker from the repo using these two commands:

```
sudo docker pull git.techmayhem.net/techmayhem/aur_coordinator:alpha-2
sudo docker pull git.techmayhem.net/techmayhem/aur_worker:alpha-2
```

Then set up a `docker-compose.yml` file, ideally in a new directory, with the following contents:

```
services:
  coordinator:
    image: git.techmayhem.net/techmayhem/aur_coordinator:alpha-2
    environment:
      BUILDER_IMAGE: "git.techmayhem.net/techmayhem/aur_worker:alpha-2"
      PORT: 3200
    volumes:
      - ./container/output:/output
      - ./container/config:/config
      - /var/run/docker.sock:/var/run/docker.sock
    privileged: true
    ports:
      - "3200:3200"
```

For additional configuration options, [see here](CONFIGURATION.md).

Finally run `sudo docker compose up -d` to bring up the coordinator. Using `sudo docker compose logs` you can check if
it managed to start without any errors.

# Usage

## Pacman

Append the following at the end of you `/etc/pacman.conf` file to configure pacman to work with the repository the
coordinator is going to create. Replace `localhost` with the address of the machine the coordinator is running on and
adjust the port if you changed it.

```
[aur]
SigLevel = Optional TrustAll
Server = http://localhost:3200/repo
```

## Building packages

On its first run archie must be setup using `archie init`. Afterwards `archie add <package>` can be used to command the
coordinator build that package. Using `archie remove <package>` the package can be removed from the repository again.

After a package has been built by the coordinator, it can be installed like any other package via pacman. So
`sudo pacman -Sy <package>` should do the trick.

`archie status` can also be used to query the current state of the coordinator.
