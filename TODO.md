# Untested

The following new features are untested:

- Packages that fail to build getting three attempts at rebuilding before timing out for a while

# Separation of concerns

The overhauled messaging systems should make some things easier. However, things bleed a bit together in terms of what
part of the application is responsible for what. I think right now things are fine, but there might be some messages
that are unhandled or even handled twice. This not being easy to check is a big issue.

Maybe I need to rethink the whole approach? Separating the various functions of the application out is good, but I just
need to work on how those different parts communicate with one another and make it very easy to follow how they inform
one another of state of things and what triggers them to do certain actions.

I should work out these modules of the application:

- Web Server
    - Takes care of communicating with the cli and the worker unit.
- Repository
    - Manages artifact files and the repository state.
- Orchestrator
    - Spins up containers for build tasks and monitors their lifecycle.
- Scheduler
    - Keeps track of managed packages and issues initial build and rebuilds (due to updates or failed build).

# Documentation

I need to write code documentation so it's more clear what each part actually does.

Also, setup instructions are needed for others to actually be able to make use of this.

# Future

Things I want to implement down the line:

- Dependencies for packages coming from the AUR also should be built. They should be marked as dependencies so that when
  the packages that they were added with gets removed they can be removed too.
- The CLI tool should be a bit of a pacman wrapper
    - If a package gets removed it should ask whether to invoke 'pacman -Rs \<package\>'
    - When package gets added, there should be the option to invoke 'pacman -Sy \<package\>' when it finished being
      built