- high prop:
    - wasm-graphics-manager must clear textures & buffers
    - entities (rendering)
    - pistol animation fixen
    - tee rendering (eyes), color creation, tee metrics (done, untested)
    - logic cleanup/refactor (happens continuesly)
    - logic splitting:
        - players hold information like is_happy etc. => pipe of a character should get this information <= character doesn't know about a player
            - player holds information that rarely changes
        - characters/entities: hold information that changes constantly
        - entities like projectiles don't hold owner information directly (pipe of proj. gets the character as valid reference <= move logic into the world logic for simplicity)
    - rav1e + https://github.com/rust-av/matroska + (https://crates.io/crates/vorbis-sys) <- c lib

- medium prio:
    - ingame menu (hässlich)
    - dummy/multiple sessions connecten (input fehlt)
    - prediction -- input is still sent too often
    - flush_vertices is a mess (and probably not correctly implemented, e.g. if vertices are "full")
    - test websockets
    - character core implements serialize & encode / deserialize & decode over the network core. Is this stupid?
    - containers should load png into gpu memory in thread already (memory needs to automatically free itself if dropped for this to work)

- low prio:
    - editor mouse events
    - editor rendering key frame points
    - sdl2 don't depend on bundled for linux
    - dbg_* config vars should not be saved
    - counting index should not use a Rc in release build
    - render order particles:
        ```
            &m_Particles.m_RenderExplosions,
            &m_NamePlates,
            &m_Particles.m_RenderExtra,
            &m_Particles.m_RenderGeneral,
        ```
    - use only one watcher and their events? probs saves some bytes, make them async?
    - vulkan memory alloc fail recover must be handled in higher levels (mostly finished, the reallocation is missing in some places)
    - split config into logical parts, gfx own config, cl own config etc. (?)
    - fix "reading image files" perf in client_map
    - create invalid test cases for network (also check and create is_0rtt tests)
    - blur causes weird black artifacts (can be seen on ctf1)
    - maybe switch to blake2 instead of sha3?

misc:
- dummy/bot icon in scoreboard/server browser
- connect as spectator (not joining the game after connecting)
- timeout code should be sent at connect already -> no blocking if max_connections_per_ip is hit
- should timeout code even be part of the network stack directly?

editor tee (animation):
- menu (file menu etc.)
- left panel
- activity bar (left from left panel)
- bottom panel:
    - animation key frames (as dots)
- center panel:
    - the animation itself
    - a panel for position, rotation etc. (to edit by typing)

- todos step by step:
    open the game:
        - only vulkan available => wgpu support (or vulkan software?)
    see server browser:
        - ui missing completely
        - http requests for master server missing
    connect to server:
        - ui for connecting to ...
        - ui for queue (when server is full)
    joined server:
        - motd (optionally)
        - first camera position should be centered on the map or smth (uses a spawn for now)
        - possibility to join spec instead of game
    network:
        - ip bans (tests & lib already exists), integration missing
        - compression
        - certs check over master server
    gameplay, vanilla (assuming not joining spec):
        - team spawns <--> normal spawn (and fallback if teamplay but no team spawns)
        - spawn particles
        - cursor (split from hud)
        - hud
        - mouse grabbing -- macos missing
        - kill tiles
        - weapons
        - weapon switch
        - hooking -- rendering buggy
        - health, shields
        - kill messages
        - points, race time
        - team points
        - win check
        - pickups (weapons, shields, hearts)
        - ninja (leave out for now, needs the new character state idea)
    server not responding/network lost etc.:
        - Needs a broadcast like string ("Connection lost...")
        - After timeout -> show UI to reconnect
    social:
        - chat
        - friend list
        
- rendering(most stuff is semi finished, but thats the first step) #1:
    - hook
    - player
    - gun
    - player eyes
    - entities (rendering)
- missing:
    - shotgun
    - laser
    - grenade
    - hammer
    - weapon swizzle
    - hammer and ninja animations
    - ninja / states
    - ctf flags
    - map entities
    - projectiles
    - hud
    - particles (manager exists, not added to client)
    - emoticons
    - user input direction arrow (?)
    - strong weak indicator (?)
    - scoreboard
    - server browser
    - settings ui
    - console
    - nameplates (discuss buffering!)
    - votes
    - motd

- rendering buggy:
    - animation evaluation
