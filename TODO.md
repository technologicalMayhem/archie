# Issues

- If no packages have been built, there is no database file in the repository. Trying to let pacman update its databases
  in this state causes it to error out.
- If the user changes the name of the repository, the old one will stick around.
- Update check interval to config file
- If an AUR package updates whilst it being built by a worker, after the worker finished the build time will be greater
  that the update time
- Let archie force a build for a package

# Documentation

I need to write code documentation so it's more clear what each part actually does.

Also, setup instructions are needed for others to actually be able to make use of this.

# Future

Things I want to implement down the line:

- The CLI tool should be a bit of a pacman wrapper
    - If a package gets removed it should ask whether to invoke 'pacman -Rs \<package\>'
    - When package gets added, there should be the option to invoke 'pacman -Sy \<package\>' when it finished being
      built
