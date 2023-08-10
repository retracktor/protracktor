# Protracktor

- A cli/sdl sound based player for protracker modules (this might get extracted into a separate crate at some point)
- Eventually: A retro handheld version of protracker
- A playground for tracker engine exploration and game controller based tracker UIs

## State

Loading and parsing the module is almost done

- [ ] Load samples
- [ ] generate static tables
- [ ] playback

## Credits

Protracktor's sound engine is based on [Tammo Hinrich's tinyMOD](https://github.com/halfbyte/ct2/tree/master/src/tinymod.cpp) and my own [CoffeScript adaption](https://github.com/halfbyte/ct2/blob/master/app/assets/javascripts/player.coffee)

## License

Licensed under [MIT license](LICENSE)
