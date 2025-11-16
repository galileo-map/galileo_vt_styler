Simple application to configure styles of Galileo vector tile layers.

To run the application, set the `VT_API_KEY` environment variable and run it with `cargo run`,
or create an `.env` file in the root of the project and use `just run`.

You load MapTiler style sheets (example files are int the `src/maptiler_style/tests` folder) and/or
manually adjust the style definitions.