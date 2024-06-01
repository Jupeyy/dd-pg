- high prio:
    - editor (blue prints, coop)
        - change on disk: suggest reload
        - timeline:
            - time control:
                - hold shift => go in steps of 100 milliseconds or smth like that
            - animation control
            - predict the position of the next animation point visually (e.g. for quad pos, where the quad will be)
        - descriptions (markdown descriptions)
        - quad slice etc
        - sound layer
        - finish auto mapper
    - dummy controls
    - dummy connect ui (as mini screen etc.)
    - video rendering (too hard for now with pure rust libs?)
    - language id for client
    - demos
    - console, rcon
    - binds
    - quic disallow "migration"? (server config)
    - bin_diff for input, send dummy input in same packet, send older input? <- does it really make sense without prediction margin
    - master server url hardcoded
    - clipboard not working on x11 :/
    - add random bytes sent by server to sign by the client (include in server info or smth) ("more secure login")
    - chinese font for chat, scoreboard etc.
    - chinese monospace font

- Accounts (managed ones):
    - password:
        - 16 chars long, first half must be strong, second half must be strong, both half must be different (second half is sent hashed)
        - alternative: just hash on client and that is secure enough?
    - private key:
        - user password encrypts private key with full password -> sends to ddnet
    - fake email:
        - a managed account generates a "fake" email that the client can use to register in game servers
    - account-login:
        - email:
            - user inputs email, long password & client generates a key-pair for session, client automatically only sends first half of the long pw
            - account-server stores session's pub key for the session (if pw & email correct) -> send back the encrypted account private key and pub key
            - client decrypts the accounts private key using the long pw and verifies the accounts pub key fits to the private key to verify the login was correct
            - client encrypts the accounts priv-key using the session's priv-key -> sends to server
            - server encrypts the previously sent encrypted-priv-key again with a private key the server generates, server sends the double encrypted priv-key to client & the pub key of the private key the server generated (TODO: or a salt/password)
            - client can now decrypt the private key in memory, while still keeping the double encrypted key on persistent (on disk), using it's sessions priv-key + servers pub-key, the server's pub-key is only stored in memory(not persistent)
    - account-register:
        - email:
            - user inputs long-pw
            - user generates priv-key (and its pub key) encryptes the priv-key it using a long-pw (see section password above)
            - user inputs email
            - client sends email and first half of long pw and the encrypted priv-key and its pub key
    - session, hardest part:
        - the client uses the session's private key to auth on server
        - server sends back the secret to decrypt private key
        - client can decrypt locally stored private keys
    - game-server register:
        - client adds the managed account fake email and is automatically registered
    - fallback mechanism (if acc server is down):
        - the client could also store the encrypted private key on client (would require to input password every time a "game-server register" happens),
            then it could simply use the long-pw to play the game even if account system is down

- round 2:
    - prediction timer must be better at start + when laggs occur
    - zstd instead of gzip? (no pure rust encoder lib yet) - brotli wins, but very slow in encoding
    - consider naga_oil's module imports for code cleanup
    - msaa & switching passes need testing and thinking -- needs mostly testing now
    - freeze/ninja concept
    - screenshot path
    - editor uses u8, other normalized format? fixed points without exp
    - network `saturating_sub`
    - should cursor handling use / 32.0? would change max distance etc. zooming in cpp version "changes" the max distance too (if 1 unit = 1 tile)
    - dmg indicators clamp is missing, maybe particle ext? - kinda finished
    - particle manager in split view adds too many particles(e.g. ninja pickup)
    - editor:
        - timeline:
            - value graph
            - hover over point => set time dragger directly to point time (easier handling)
            - values are often off (because inner graph rect is often not used correctly)
        - tune layer should have tune zones as property, also how exactly does old ddnet handle this?
        - hover over image (when changing tile layer image) -> preview layer with that image -- "raw" image preview missing
        - map versioning - all fine?
        - decouple anim time from editor time (so during animations -> use anim time) -- all fine?
        - move quads => insert key frame at current pos - all fine?
        - move timeline time => set quad to current predicted pos - all fine?
        - host, join ui - ugly, doesn't save anything previously typed in
    - v assets download from server (http server), layered containers? - needs testing
    - ^ resources / containers refactor:
        - resource suppliers:
            - server
            - resource https server
            - local files
        1. JSON from resource https server that contains all downloadable items
        2.  no hash mode:
                - download latest version from resource https server => save with hash and "null" hash (filled with zero?)
                - if not found => load file without any hash
            hash mode:
                - load from disk using the hash
                - if not found => load from server

- round 3:
    - vk testing: use the plugin interface to check command correctness (?)
    - server is empty => auto reload config (if changed)
    - uploading objects to wasm module uses wasm func call
        (rewrite to change linear memory)
    - console ideas:
        - show a "span" for multi argument commands, to easily identify where a command started and stopped
        - show the current value already when typing (for all suggestions)
        - description for commands/variables (use rust doc comments and add to the derive macro)
    - vk custom render:
        - wasm support
        - load custom shader files
    - video rendering plugin:
        - offscreen canvases support (but how?)
        - offscreen id must be returned by graphics impl
    - when vk swapchain fails, it requires either the old swapchain or a panic (window.rs -> create_swapchain)
    - descriptor layout cleanup
    - failed network connect -> close client -> poisioned mutex (on_data(Err(anyhow!(err.to_string()))).await.unwrap();)
    - input (fire&hook etc.) should also store cursor pos
    - brainstorm particles + sounds etc.:
        Q: How should the state tell about sound effects?
        P: Weapon switch
        I: Keep events. How are sounds synchronized?
        I: Only generate sounds for local players client side? (Else wait for server)
    - hud (connect to game state)
    - particles (idea, connection to game state etc.)
    - pistol animation fixen
    - logic cleanup/refactor (happens continuously)
    - logic splitting:
        - players hold information like is_happy etc. => pipe of a character should get this information <= character doesn't know about a player
            - player holds information that rarely changes
        - characters/entities: hold information that changes constantly
        - entities like projectiles don't hold owner information directly (pipe of proj. gets the character as valid reference <= move logic into the world logic for simplicity)
    - rav1e + https://github.com/rust-av/matroska + (https://crates.io/crates/vorbis-sys) <- c lib
    - hold f3 for few seconds in vote -> force vote (if mod)
    - proc macros need fallback if parsing fails
    - DeserializeSeed for wasm results & parameter decoding (saves heap allocation for e.g. `Vec<PlayerRenderInfo>`) in hot path

- medium prio:
    - tee rendering (eyes), color creation, tee metrics (done, untested)
    - entities (rendering)
    - ingame menu (ugly)
    - dummy/multiple sessions connecten (input fehlt)
    - prediction -- input is still sent too often
    - flush_vertices is a mess (and probably not correctly implemented, e.g. if vertices are "full")
    - character core implements serialize & encode / deserialize & decode over the network core. Is this stupid?
    - containers should load png into gpu memory in thread already (memory needs to automatically free itself if dropped for this to work)
    - use some URL container for url strings (instead of string directly)

- low prio:
    - test websockets
    - editor other user's mouse events
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

- misc:
    - dummy/bot icon in scoreboard/server browser
    - connect as spectator (not joining the game after connecting)
    - timeout code should be sent at connect already -> no blocking if max_connections_per_ip is hit
    - should timeout code even be part of the network stack directly?

- editor tee (animation):
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
        - http requests for master server missing (getting list works, master server not working, registering missing)
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
    - shotgun
    - laser
    - grenade
    - hammer
    - weapon swizzle
    - hammer and ninja animations
    - projectiles
    - particles
    - scoreboard
    - server browser
    - console
- missing:
    - ninja / states
    - hud
    - emoticons
    - user input direction arrow (?)
    - strong weak indicator (?)
    - hookline
    - settings ui
    - nameplates (discuss buffering!)
    - votes
    - motd
