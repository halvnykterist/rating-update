# rating-update
Ratings for Guilty Gear: Strive

## How does this work?

Replays are pulled from Strive's servers using [ggst-api-rs](https://github.com/xynxynxyn/ggst-api-rs). They are then processed using a modified Glicko algorithm, which you can read more about [here](docs/modified-glicko.md).

## Customizing
Download [Bulma's](https://bulma.io/) sass source files and place the contents in /static/sass. Use the sass executable via npm to generate styles **.css** from styles. **scss**.


## Setting up a local database for development

To setup a database with some data you can run the following commands.

```bash
cargo run init # Setup the tables and indices of the database
cargo run #Start pulling matches and updating players.

#For release mode (faster, but slower to compile)
cargo run --release init
cargo run --release
```


Some other useful commands available are:
```bash
cargo run nothoughts #Will only run the website, without updating any data
cargo run pull #Pulls data, without updating anything
```

You can find more in `main.rs`


Once the database is setup you can start a local server that is accessible on `localhost`
with `cargo run`. By default the server will continuously pull down new replays and update the rankings. If you do not
want this behaviour you may run `cargo run -- nothoughts` instead to only start the website.
