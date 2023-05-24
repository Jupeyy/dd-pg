source:
 - client
    - graphics
    - input
    - audio
 - server
    --- should not exists as much as possible, since it should be merged with shared code with the client
 - shared
    - game/simulation
    - network
    - io
    - logging
    - system
    - mapfile

pipelines:
```
Pipelines are a concept to cleanly split events, in a sense of what happens after, of the overall logic.
Pipelines are also an interface that the functional component objects can use to gather and store information.
This makes all the pipeline components more unique and modular, because components should only know as much as they need to know.
The components itself don't need to and should not store mutable references to other components, instead the pipeline passes all required information/components. This makes the whole concept more "rust-like". It can also enforce some rules that can benefit the underlying backends by forcing the higher level components to only allow certain actions in certain pipelines.
It's quite possible some pipelines have subpipes to enforce even stricter rules for specific components, e.g. the menu background map rendering
does not need to know about the game logic at all, so it also shouldn't know about it.
The main pipelines are:
 - Events: Pull network and handle events
 - Input: Pull input and apply it to all input listeners (maybe even for server TODO)
 - Simulation: Advance the game tick (or prediction tick for the client) and update any kind of game states
 - Rendering: Render graphics
The main purpose is that each of these pipelines should rely on the state of the previous pipeline,
but should never do what the previous pipeline is able to do.
For example the "Rendering" pipeline can only make render calls, but it can never create new buffer objects.
In order to create buffer objects, one has to do it when it makes sense, mostly when an event triggers a change,
e.g. if a user uses a new skin and the skin has to be loaded into the graphics memory.

"Input" comes after the "Events" so it has a higher priority to overwrite any kind of change triggered by the Events pipeline
```

todos/ideas:
Pipelines:
 - how to make the simulation & events pipelines shared between the client and server as much as possible.
   Events on the client should have access to the graphics, while the server does not require this.
   Should the Simulation have access to the graphics on the client?

 - Events:
    - pipelines interface:
    - logic structure:
 - Input:
    - pipelines interface:
    - logic structure:
 - Simulation:
    - pipelines interface:
    - logic structure:
 - Rendering:
    - pipeline interface:
        - system: time, logging
        - client: ?
        - game: game states (e.g. current tick)
        - map: information about the current map data. Even tho this is often strongly connected to the game logic its still a different concept, since not all components need the full game logic to operate (and maybe shouldn't be able to use the game logic)
    - redering structure for the client:
        - if game is not in "game server" or "demo" mode it should always render the map background
        - if "game server", render the background of the map
        - if "game server", render all primary components (skins, effects, hooks etc.)
        - if "game server", render the foreground of the map
        - if "GUI open", render the GUI


Client:

Server:

Server/Client interoperation:
 - Server code should be callable inside the client's process. There should be some kind of GUI hook then, so the server can display certain
   information directly to the users screen (in the client, or maybe even a server start parameter)

misc/unsorted/uncategorized:
 - skins:
   - make it more modular (example default skin):
      - skin/default is directory:
         - images:
            - body.png
            - ears.png
            - eyes.png
            - marking.png
            - decoration.png (ears basically^^), maybe multiple?
            - hands.png
            - feet.png
         - animation, animations that are used for the tee at various tee states:
            - idle.ani
            - walking.ani
            - running.ani
            - walking.ani
            - jumping.ani
            - double_jump.ani
            - hook.ani
            - fire_pistol.ani
            - fire_granade.ani
            - fire_shotgun.ani
            - fire_puller.ani (this is basically the ddrace shotgun)
            - fire_laser.ani
            - fire_ninja.ani
            - fire_hammer.ani
         - effects, there are different kind of effects, permanent effects, state effects:
            - permanent effects:
               - tournament_winner.eff
               - tournament_second.eff
               - tournament_third.eff
               - donator.eff
               - game_moderator.eff
            - state effects:
               - none.eff (no state)
               - ninja.eff
               - frozen.eff
               - glowing.eff
               - poisoned.eff
               - buffed.eff
               - sleeping.eff
               - wet.eff
               - sweeting.eff
               - burning.eff
               - ghost.eff needs at least 3 modes: invisible, transparent, semi(e.g. the dots in ddrace). (mode where tee is not touchable etc.) /spec for ddrace, maybe useful for other mods
            - status effects:
               - spawning.eff
               - dieing.eff
               - fainting.eff (same as dieing?)
               - freeze.eff
               - unfreeze.eff
               - damage.eff
               - catching_fire.eff
               - extinguish_fire.eff
            - server status effects:
               - rampage.eff
               - killingspree.eff
               - unstoppable.eff
               - dominating.eff
               - whickedsick.eff
               - godlike.eff
               - afk.eff
   - explainations:
      - images: Simply .pngs (maybe other formats, but NO GIFS. nothing frame based)
      - animations: Key frames for feet, maybe the unused hand(s)?, whole tee (e.g. moving up down), what about eye blinking?
      - effects: effects are a format that can enhance the tee's appearance, they can generate particles, which have envelopes and RNG if wanted. the different effect groups can be displayed all at once, but each group (except the permanent one) itself can only have a single state?(easier to use, but needs discussion/brainstorming):
         - permanent states are rendered first, set by server (maybe overwritable by client, but not really useful except debugging?).
         - state effects are second
         - server state third
         - status effects last
         - permanent states are meant to enhance visual appearance generally
         - state effects are meant for enhancing the state a tee is in (frozen, ninja etc.)
         - status effects are basically event triggers(gets damanged, spawning etc.). status effects are basically short effects that are on top of the tee, differnt to the states, which are more like a "current" permanent effect for a tee
         - server states are special states which are not directly related to the tee states but are also more "permanent" than short effects.

Brainstorming:
   - Events:
      - Network thread:
         - pull network packets
         - generate game msg
      - Game thread:
         - check for game msg
         - send game msg to registered components (with pipe)
         - pipeline brainstorming:
            - structure:
               - game state, to edit it...
               - graphics, to add/rem buffer objects
   - Simulation:
      - How to connect teams into the design:
         - don't use MAX_CLIENTS for iteration, instead use vectors? too slow? maybe pure forward lists, that hold a skip_index for fast iterations <---
         - generally no const size arrays? check ram usage maybe, for most stuff its maybe just pointers, so should be ok
      - game state:
         - design:
            - teams/stages: basically that the same world can be used multiple time (ddrace teams):
               - world:
                  - team/stage index
                  - entities:
                     - team/stage index
                     - types:
                        - characters
                        - projectiles:
                           - have an owner ID (-1 or smth for none). it's important to sent this in the network code too (vanilla didn't do this, very annoying ^^)
                           - types:
                              - gun projectile
                              - laser projectile
                              - puller projectile (ddrace shotgun)
                              - grenade projectile
                              - door laser
                              - shotgun projectile
                        - flags:
                           - red
                           - blue
                           - general/custom color?
                        - pickups:
                           - types:
                              - grenade
                              - shotgun
                              - laser
                              - puller (new, replaces ddrace shotgun)
                              - ninja
                              - pistol
                              - hammer (tees should be able to carry nothing at all, differently to vanilla)
                        - weapons (and some reasoning):
                           - grenade (just like ddrace)
                           - shotgun (like vanilla), this is not included in ddrace
                           - laser (like vanilla, not included in ddrace), for ddrace laser is so hot that the ice melts
                           - puller (like ddrace shotgun), this is a new concept for vanilla too now, since its a new weapon
                           - ninja (like vanilla and ddrace)
                           - pistol (like vanilla and ddrace)
                           - hammer (like vanilla and ddrace), for ddrace hammer breaks ice
            - entity core changes:
               - entities now always define a specific struct object.
               - This object contains all attributes that should be shared between ticks, the entity core is always part of this:
                  ```rust
                  struct CharacterCore {
                     entity_core: EntityCore, // <-- must have this
                     freeze_tick: i32, // just an example of an attribute that is shared between ticks
                  }
                  ```

                  - the reason for this is that an entity now has to define an array of exactly two core struct objects. These core struct objects have a specific purpose:
                     - the old/previous core is read-only and can be used in the current tick to e.g. advance the position
                     - the current core is write only, so you have to use the old core to make your changes and then apply it once you finished
                     - this allows to use these core objects for prediction in the client and still can be shared with server code. The server will simply get the same core for read/write operations

- cores (entities) brainstorming:
   - for prediction cores always reset to the previous core (non prediction core)
   - ??? <- there should be properties (heap objects (no Copy trait), which are only changed during a "real" tick and never by a prediciton tick), removing entities from world 
   - if not above, prewarmed buffers only -> no heap allocations... ez for Vec, but for std::set? maybe disallow such datatypes then
   

- map feature: design tile layers can scale their tiles
- ui controlled by server
- how to implement sql cleanly?
- what ui to pick? HTML UIs arent ready yet :/, egui?
- votes per team, vote color
- implement logging directly with colors in mind
- chat improvements? maybe chat under name, more colors: when typing in team -> show only team msg, typing in global -> only global, whisper -> whisper etc.
- whisper msg must be encrypted!
- split screen, also respect it in network packages directly
- hud(including chat) ui items draggable over screen (maybe save in percentages)
- connecting queue for full servers

- dummy clean implementation brainstorming:
   - best would be if there is only a `this_client_id` that needs to be changed
   - network is a problem for above solution. else it might work
   - copy/hammerfly etc. must be a seperate input handler

- community server brainstorming:
   - maybe implement connecting queue for full servers here? but also sucks for non ddnet servers
   - accounts
   - friend list
   - friend goes online offlihttps://freemasen.github.io/wiredforge-wasmer-plugin-code/part_2.htmlne
   - sync settings?

- network brainstorming:
   - error handling:
      - ?
   - connect:
      - disconnect old session
      - already sent as much as possible in first packet?
   - disconnect:
      - ?
   - fix bug with map changes in ddnet
   - generate game msg in thread async
   - use relaxed atomic bool for communication
   - support a sleep command: sleep_until_or_packet_arrived
   - unintendeted disconnect -> game msg
   - errors -> game msg
   - check all packets in this thread for correctness already (except its impossible)
   - simplify compression? struct -> compression. But always use structs for packets, so they are easier to handle without some NetUnpacker stuff
- game msg brainstorming:
   - simply uncompressed network packages?

- skin swizzle brainstorming:
   - opengl 1 needs some kind texture copy function for all skin parts
   - maybe this should imply that skin texture indices are known are draw time by the game state, instead of ever calling `findskin`

spectator brainstorming:
   - ddrace has a freeview and /spec
   - in vanilla spectator is a new team
   - /spec should maybe be a general tee state?
   - /pause would be a "allow freecam"?
   - vanilla spectator would thus still exists
   - /pause -> free cam, tee stays untouched
   - /spec -> free cam, tee goes into ghost mode

- reverse proxies directly into the network implementation
- meta data tile. e.g. to know where to take automatic screenshot

some impl details:
   - 1 event handler registers to exactly one event -> just register a global function? probs no:
      - problem: what about stateful managers, so better register a struct with a trait
   - 

- collect statistics:
   - player moved (left right)
   - player jumped
   - player hooked
   - player state times
   - player afk time
   - player emoted

client v2:
- no component driven approach (components in a sense like vanilla)
- wasm where possible, how to give it access to the graphics API?

flow:
   - network
   - input
   - update
   - rendering (when needed)

update:
   - how without components? should components exist, but not be related to rendering?
   - maybe in a first version no wasm at all?
   - physics only changable over tunings (not wasm)
   - what about components like stats?

rendering:
   - background - wasm?
   - world:
      - background - wasm?
      - players/entities/particles etc.:
         - players:
            - hook
            - weapons
            - tees
         - entities:
            - weapon pickups
         - particles:
      - foreground - wasm?
   - HUD
   - UI:
      - should have a PATH similar to browsers, that allows easy integration of wasm, probably?


