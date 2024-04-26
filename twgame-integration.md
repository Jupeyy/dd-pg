Note: this is currently a "draft-issue". Early suggestions welcome. I want to think a bit more before we actually handle this issue.

This is a todo list and the current plan to integrate twgame into the "ddnet-next", which I call dd for the rest of the issue, ecosystem.  
As a start dd will depend on twgame, twgame however must depend on the interface of dd. This isn't optimal, but i don't see any other way. For the start it's ok I guess.


### Todo list:

- [ ] map loading -> convert to twgame
- [ ] player join, player leave, player input, player info
- [ ] collect render information
- [ ] user commands (kill, chat commands etc.)
- [ ] rcon commands
- [ ] ticks & prediction ticks
- [ ] snapshots, reading from snapshots
- [ ] events
- [ ] misc (camera start pos)
- [ ] some planing of what would else be useful


### Additional notes


Most stuff is documented in the interface, however I want to make very clear again that snapshot deltas do not exist, they are created in the outside implementation automatically (if at all).  
Additionally "hacks" like having a tick in the character snapshot that advances based on a previous snapshot does not work. A snapshot must write everything it needs and must be able to recreate the full world from a snapshot.  
Events like explosions etc. are NOT part of the snapshot anymore, the client tries, as good as possible, to predict sounds etc.  
The events must be transparent to the client (the client has to know about what events are, differently to snapshots).  
Additionally events are a bit different to ddnet events, they also contain kill feed/action feed and system messages (player join etc.) directly. But such stuff should get obvious at implementation time.
  
### Implementation notes


Generally I'd offer to implement the rendering stuff, since I think I know what i need and can then also extend rendering stuff by need of ddnet.
What I really would appriciate if you, Zwelf and/or Patiga, would take over the snapshot part with the above requirements in mind,
you are not forced to use ddnet's snapshot system. In theory if your implementation does not used shared pointers, it might even be possible to just serialize and deserialize everything :D.  
  
Anything outside the interface is ofc not your problem, if you have problems, I'll fix them.  
  

### Considerations


Some additional long-term considerations:
- Switch to a weak copy left license like MPL or permissive license
    This would allow me to not relicense my project and ship/distribute the whole project under a similar license ddnet is using now.
    Additionally closed-source anti-bot systems which are combined with the server create a service and thus would violate AGPL, because that is a form of distribution. This is different to how a end-user might load a AGPL plugin (not shipped by the client), because he does not create a "service"/distribution. This is all very complicated and I also don't fully understand everything, but I think you can kinda imagine it similar to how a browser might load a AGPL website^^
    This is ofc completely your decision and I respect any kind of decision pro or against dd
- If the project seriously goes live, we should also discuss if we switch to the ddnet-repo on github, I'll clarify that with deen/heinrich. I know you like gitlab, but I could assume most current and future ddnet contributors are more likely there. This is just a future keep in mind, we can always discuss this step.