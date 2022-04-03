# rating-update
Ratings for Guilty Gear: Strive

## Customizing
Download [Bulma's](https://bulma.io/) sass source files and place the contents in /static/sass. Use the sass executable via npm to generate styles **.css** from styles. **scss**.


## Setting up a local database for development

To setup a database with some data you can run the following commands.

```bash
cargo run -- init # Setup the tables and indices of the database
cargo run -- pull # Pull down replays from the GGST API
cargo run -- update # Update the ratings
```

Once the database is setup you can start a local server that is accessible on `localhost`
with `cargo run`. By default the server will continuously pull down new replays and update the rankings. If you do not
want this behaviour you may run `cargo run -- nothoughts` instead to only start the website.
