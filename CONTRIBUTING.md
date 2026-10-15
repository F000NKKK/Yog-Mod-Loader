# Contributing to Yog-Mod-Loader

Thanks for wanting to help with the Yog mod loader: Rust crates and the Fabric, NeoForge and Forge hosts. Bug reports, ideas, documentation
fixes and code are all welcome.

## Reporting a bug

Open an [issue](https://github.com/F000NKKK/Yog-Mod-Loader/issues/new/choose) with:

- what you did, what you expected and what happened instead;
- your operating system, Minecraft version and loader (Fabric, NeoForge or Forge);
- the log or a screenshot, if there is one.

Search the existing issues first — someone may have reported it already.
Security problems are not reported in issues: see [SECURITY.md](SECURITY.md).

## Suggesting a feature

Open an issue describing the problem you want solved before writing a lot of
code, so we can agree on the approach first.

## Making a change

1. Fork the repository and create a branch from `master`.
2. Make the change, with tests where it makes sense.
3. Check that everything builds and the tests pass:

   ```bash
   ./build.sh test          # the Rust crates' tests
   ./build.sh               # the loader jars (JDK 17 and 21 are picked per Minecraft version)
   ```

4. Open a pull request that explains what changed and why.

Rust code lives in rust/crates, the Java hosts in fabric, neoforge, forge and java-common. Keep the hosts thin: anything a mod can use belongs in the Rust API, and nothing mod-specific goes into the loader.

## Style

- Code reads like the code around it: same naming, idioms and comment density.
- Rust is formatted with `cargo fmt`; TypeScript follows the existing formatting.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(scope): …`, `fix(scope): …`, `docs: …`, `build(deps): …`, `chore: …`.
- The project is in active development: an outdated API is replaced, not kept
  alongside the new one.

## License

By contributing you agree that your contribution is licensed under the
project's [license](LICENSE).

## Code of conduct

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
