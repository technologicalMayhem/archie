# Coordinator

As of right now all config options for the coordinator are specified via environment variables. In a docker compose file
that corresponds to the `environment` section. The following options are available right now:

|                      Name | Default value | Description                                                                                                                                                                                                                        |
|--------------------------:|:-------------:|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
|          ``MAX_BUILDERS`` |       1       | The amount of package builder that are allowed to run in parallel. Be aware setting this higher than one might quickly starve the system of resource as compiling source code is intensive work.                                   |
|           ``MAX_RETRIES`` |       3       | After a build failed, the build will be restarted after five minutes. Should this happen more than the specified value, the build will 'time out' until the next update check interval when it will give it a set of new attempts. |
| ``UPDATE_CHECK_INTERVAL`` |      240      | The amount of time, in minutes, between querying the AUR for package updates.                                                                                                                                                      |
|                  ``PORT`` |     3200      | The port the web server should bind to.                                                                                                                                                                                            |
|         ``BUILDER_IMAGE`` |  aur_worker   | The docker tag of the image for the builder unit.                                                                                                                                                                                  |
|             ``REPO_NAME`` |      aur      | The name for the repository that the packages will be placed in.                                                                                                                                                                   |

# Archie

Archie stores it's configuration files in the users home directory under `$HOME/.config/archie/config.toml`. Alternative
profile use the name as the profile instead of `config` for their file name. Right now all options that can be set can
be configured by just running `archie init`.
