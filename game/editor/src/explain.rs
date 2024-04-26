pub const TEXT_QUAD_SELECTION: &str = 
"\
# Quad selection\n\
\n\
The quad selection is a specialized tool that focuses on making working with existing quads easier.\n\n\
You can change many shared properties at once to many quads. E.g. the color of one of the quad corners.\n\
Align many quads at once and so on similar to the [`Quad brush`](#quad-brush).  \n\
First select your quad(s) using `left click`:\n\
- `right click` on a quad corner or center _point_ to open the property\
window for all selected quads for the given _point_.\n\
- `left click` on a corner point to drag the corner of all quads.\n\
- `left click` on a center point to drag all quads.\n\
- `shift + left click` on a center point to drag the center point of all quads.\n\
- `ctrl + left click` on a center point to rotate all quads.\n\n\
The `Alt`-key will always try to snap the above actions to the [`Grid`](#grid) (if active).\n\n\
Press `right click` on no quad to unset the selection.\n\n\
### Animations\n\
\n\
If one or more quads are selected with at least one shared pos/color animation and the [`Animations`](#animations)-panel is open you can \
control the animation properties using the quads directly. In other words: instead of changing the animation point values \
inside the [`Animations`](#animations)-panel you can simply drag the [`Animations`](#animations)-panel's time dragger to where you want to insert \
the next animation point -> change your quad properties -> insert the animation point at this position.\n\
> Keep in mind that moving the [`Animations`](#animations)-panel's time dragger resets the quad to the \
evaluated position/color etc.  \n\
The property's value is the interpreted as `value = base-value * animation-point-value` \
to calculate the value of the animation points.\
";

pub const TEXT_TILE_BRUSH: &str = 
"\
# Tile brush\n\
\n\
The tile brush allows to select a range of tiles (`left click`) and apply different action on this selection:\n\
- `Left click` -> Draws this selection anywhere within a tile layer.\n\
- `Shift + left click selection` -> Creates a repeated pattern of the selected tiles.\n\n\
---\n\n\
Press `right click` to unset the selection.  \n\
Hold `space` to open the tile picker, which is basically an overview of all tiles within a tile layer image.\
";

pub const TEXT_LAYER_PROPS_COLOR: &str =
"\
# Layer's color\n\
This controls the base color of the tile layer.\n\
\n\
### Animations\n\
\n\
If the [`Animations`](#animations)-panel is open and this layer has a color \
animation active, then you can change this property and \
insert a new animation point at the current [`Animations`](#animations)-panel's \
time value (move the time dragger) instead of changing the animation points inside
the [`Animations`](#animations)-panel.\n\
> Keep in mind that moving the [`Animations`](#animations)-panel's time dragger resets the color to the \
evaluated color of the animation.  \n\
The color-property's value is the interpreted as `color = base-color * animation-point-color` \
to calculate the values of the animation points.
";

pub const TEXT_ANIM_PANEL_OPEN: &str =
"\
# Animations panel + properties\n\n\
To make animating easier to use properties that are effected by animations like position, color & sound volume \
are entering a different mode when the [`Animations`](#animations)-panel is open.  \n\
Instead of changing the properties directly it will leave the base properties as is and modifies a temporary \
property.  \n\
This temporary property is the sum/product of the base property with animations applied:\n\
- position: `temp_pos = base_pos + anim_pos`\n\
- color: `temp_color = base_color * anim_color`\n\
- volume: `temp_volume = base_volume * anim_volume`\n\n\
The conclusion of this is that if you insert a new animation key point, then this key point can automatically \
calculate the animation point values using the above equasion.\n\n\
> - You can opt-out of this animations handling in the global settings.\n\
> - Closing the animation panel will allow you to modify the base values again.\n\
> - Color values are always in the range [0-1] (or [0-255]), so e.g. if the base color for the red channel is 0 \
    the animation point will be simply 1 (because the animation point cannot magically make the final color higher than 0).  \n\
    In other words that means that the base color reduces the color range (0.5 => anim point is 0.5 at most).\
";

pub const TEXT_QUAD_PROP_COLOR: &str = 
"\
# Quad's color\n\
This controls the base color of the selected quads.\n\
\n\
### Animations\n\
\n\
If the [`Animations`](#animations)-panel is open and this quad has a color \
animation active, then you can change this property and \
insert a new animation point at the current [`Animations`](#animations)-panel's \
time value (move the time dragger) instead of changing the animation points inside
the [`Animations`](#animations)-panel.\n\
> Keep in mind that moving the [`Animations`](#animations)-panel's time dragger resets the color to the \
evaluated color of the animation.  \n\
The color-property's value is the interpreted as `color = base-color * animation-point-color` \
to calculate the values of the animation points.
";

pub const TEXT_LAYERS_AND_GROUPS_OVERVIEW: &str =
"\
# Layers & groups overview\n\n\
This gives an overview over all groups and their layers.\n\
\n\
Groups are separated into 3 main categories:\n\
- __Background__: the layers that will be rendered behind the Tees and other ingame objects.\n\
- __Physics__: the layers that control how the game works.\n\
- __Foreground__: the layers that will be rendered in front of ingame objects.\n\n\
All groups & layers can be hidden pressing the eye-symbol. This will not affect how they are displayed in the client!\n\n\
To open group & layer properties `right click` on their names. To select multiple groups __or__ layers hold `shift`.  \n\
This will allow to modify the overlapping properties of multiple groups/layers at once.  \n\
To activate a layer `left click` the name.\n\
";

pub const TEXT_IMAGES: &str =
"\
# Images (quad layers)\n\n\
Quad layers support any kind of images without special requirements.\n\
> You can still use the same image for quad layers and tile layers, simply include it in both tabs, \
the client's implementation prevents loading the image twice.
";

pub const TEXT_2D_IMAGE_ARRAY: &str =
"\
# 2D image arrays (tile layers)\n\n\
Tile layers need special images. Their requirement is that the width and height must\n\
be divisible by 16 (e.g. `1024 / 16 = 64` without a rest => divisible by 16).  \n\
> You can still use the same image for quad layers and tile layers, simply include it in both tabs, \
the client's implementation prevents loading the image twice.
";

pub const TEXT_SOUND_SOURCES: &str =
"\
# Sound sources (sound layers)\n\n\
Sound sources are simply sound files that can be played by the client.
";

pub const TEXT_ANIM_PANEL_AND_PROPS: &str =
"\
# Animations panel + properties\n\n\
To make animations easier to use, you can use the properties of e.g. quads \
to animate your quads while the animations panel is open.  \n\
That however means that the _base_ properties will not be overwritten when \
changing the properties, because instead it will write to temporary properties\
that are influenced by the current animation time and allow to insert new animation points.  \n\
To opt-out of this behavior, enable this option.\
";
