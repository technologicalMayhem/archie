# Documentation

I need to write code documentation so it's more clear what each part actually does.

Also, setup instructions are needed for others to actually be able to make use of this.

# Future

Things I want to implement down the line:

- The CLI tool should be a bit of a pacman wrapper
    - If a package gets removed it should ask whether to invoke 'pacman -Rs \<package\>'
    - When package gets added, there should be the option to invoke 'pacman -Sy \<package\>' when it finished being
      built
- Hard fail for packages. After a certain amount of attempts a package will 'hard fail' and never be rebuilt unless a
  user specifically requests it or there is an update for the package
