# Modrinth add-on
Provides a sub-command `modr` to manage mod installation in the `mods` directory, mainly used 
by Fabric and Forge mod loaders.

[**Download (1.0.0-pre3)**](https://downgit.github.io/#/home?url=https://github.com/mindstorm38/portablemc/tree/master/addons/modrinth)

## Usage
This addon is currently in development, this document doesn't describe it in details. 
You can get advanced help with the following command: `portablemc modr -h`.

This addon is based on the Modrinth API, which is an open-source mods' distribution platform.
Owners choose or not to put their mod on Modrinth so some popular mods cannot be found, but 
most recent ones like CaffeineMC's mods (sodium, hydrogen) are available. Some Forge mods 
are also available.

> Since this is in development, this addon might be split in the future to provides a more
> common API to manage the `mods` directory.

## Examples
```sh
portablemc portablemc modr -h
```

## Credits
- [Modrinth website](https://modrinth.com/)
