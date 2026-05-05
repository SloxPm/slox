# Slox
Slox is a package manager written in Rust designed to achieve speed, stability, and liquidity.

# Key Features

## Env Management Function
You can dynamically switch between multiple env profiles.

## Colorful Logs
slox's clean logs provide convenience to users.

## Customizable
The slox package allows for various customizations, such as build scripts and names, through build.toml.

## Easy
Just a single build.toml, no multiple files. You can distribute your software and let the world know through slox.

# Install
You can install slox through Cargo.
> [!WARNING]
> slox is only available in macos/linux
```bash
cargo install slox
```

# Guide

## How to install a package

```bash
slox pkg add somepkg@sloxpkgs # pkg in std-pkgs
slox pkg add someone/somepkg@github # pkg in github
```

## How to manage Environment

```bash
slox env add home
slox env set home
slox env remove home
slox env fetch 
```



