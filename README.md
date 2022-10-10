# Contours
## Scoring
Triangle adjacent to lake - bad. 
Closed contours should have a diameter of 5 m -> circumference of 30 m. 
Inflection points (how?) are good.
Many direction changes - really bad. Some are ok.
Using cliff triangle - very good.

## Identify inflection points
Window of 7 points. 
4 outer points - identify Bezier.
Sum of distance to 3 inner points must be large enough. 
Middle point is inflection point. 

- Deviate 0,5 - 1 m from global
- Locally deviate up to 1,5 m for a part of the contour (optional)
- Replace small contours with knolls
- Look 1,5-4 m up to find small closed curves and add hjälpkurvor for those
- Select one 5 m level for stödkurvor symbol.

Also need a better (or more tailored) Bezier fitting algoritm, that doesn't introduce artifacts. Is there a way to identify sänkor / näsor - these need to be control points.

Port or use bezier.c from C code.
Start with march identification? Once that is done then maybe maps are somewhat runnable. MVP.

## Water 2

Sample height onto rectangular grids 1x1 m. 2500x2500 px for one block. Can run in parallel if there are multiple blocks.

### AI model

Input:
    - height (scaled to 1 at (min+max)/2, and going from 0 to 2)
    - max height of vegetation returns in 5x5 area (divided by 20)
    - contains LAS water points (0 or 1)
    - has no ground points (0 or 1)
    - number of ground returns vs num vegetation returns (averaged over a 5x5 m area), expressed as a quotient (vegetation returns / (ground returns + 1)).
    - standard deviation of 

Output:
    wetness (0 not wet, 0.25 diffuse marsh, 0.5 marsh, 0.75 impassable marsh, 1.0 lake)
    open 0/1
    green (0 not green, 0.5 light green, 1.0 dark green)
    road 0/1
    building 0/1
    ditch 0/1
    stone wall 0/1
    smoothness (0 - farmland or paved road, 1 - anything else)

Use hexes, and provide 90 + 1 surrounding points. That's 5 m out. 364 input values. Training data can be augmented 6 times by rotation, and another 6 times with mirroring. 

Fully connected network. 2 hidden layers, 120 and 40 nodes each, and 8 outputs. `91*5*120 + 120*40 + 40*8 = 59720` weights. This seems like a very reasonable number of weights. 

Training data: "painting" the correct classification based on LAS data + old maps. Data can be augmented by rotation and mirroring. A large challenge is that some features may not be properly aligned on the old map. Requires hand-painting each feature. 2500m side means 1e7 pixels to classify. Important to have a "fill" tool and an adjustable brush size.

Maybe do a second step, with the classification from the previous step as input, but looking over a larget 10 m (331*8 inputs). 

# Training generator
Each hex is represented with 6 triangles. Each corner vertex is the average of the three hex centers. 
Display a roughly 25x25 m large 3D cutout, that can be rotated, and the corresponding OCAD texture flat, to the side. Each hex needs to be painted with one of the following colors:

- diffuse marsh
- marsh
- open diffuse marsh
- open marsh
- impassable marsh
- lake
- farmland
- open land
- light green
- dark green
- dirt road
- paved road
- stone wall
- ditch
- building

There will be 100*100 = 10000 such cutouts. 1 per minute, 50 per hour means 25 days at 8 hr/day. Summer job. Can also hire some other kid (but must know orienteering maps). Fritjof can do Guddehjälm or old SM 2013 map. I can do Fontin. Kalle, Wilma, m.fl.

Can generate 625*12 = 7500 training points per minute. Need at least 5M, that means 666 minutes = 11 hr. Very reasonable.

The training generator needs to be written in Swift as a Mac GUI app.

Generate inputs per hex point (for circles around each hex center):
 - calculate a list of hex center points for a given bounding box.
 - generate data for each center point
 - 

Add command-line argument to snabbkarta for saving an intermediate file with input data. "map_ml_input_data". 

### Language

Interoperability with CoreML is important. We can call Objective-C methods from Rust, and Metal / CoreML has an Objective-C API.

### Parallelization

We can run the ml generation first, which can operate on smaller dtm blocks. Then inference can run in parallel to the full DTM generation.


# Cliffs

Cliffs are often "broken" by triangles that are too flat. Allow more flat triangles when growing, but only in the direction of the cliff. Perhaps we should keep track of a plane continually while growing the cliff.

# General boundary improvements

Keep track of the total area. This way we can enforce a minimum requirement on area.

# Water model

So, the water model didn't work out; manages to somewhat detect edges of marshes, but not well enough.

Instead, attempt to locate areas that are flat. Distinguish between "diffus" and "normal" by height of trees.

Build upon seed triangles, as long as z value is within 0.3 m. 

Assign water quantity to each triangle in proportion to area.
Find the recipient triangle for each triangle.
Move water out in proportion to z-normal.
If quite flat, let some water out the wrong ways (to avoid small "spurious" holes), unless recipient triangle is a 
lake triangle.
Keep track of sum of received water (this excludes the first water qty).

Perform this step after lakes, where we know if each triangle is assigned to a lake or not.

Run a grow-boundary / lake thing where we look for triangles with sufficiently high sum of water flow. Exclude lake triangles from this.
Then run a second boundary thing where we have a lower threshold. Exclude lake and heavy bogs.
We can do this with three boundaries, for heavy, normal and diffuse marshes. 

This should actually be fairly quick now!

Streams - are they extra heavy? 

Start one area at a time -> we don't need to merge two areas.

OSM or FAST threads should post streams (as Vec<Sweref>)
All triangles intersecting these lines should be cast as Terrain::Stream. Not just corner points, use interpolation between Sweref points.

# OSM interface

Terrain triangles should be marked as "road" or "stream" from OSM. Or "building". 

Post objects to main thread. Main thread waits until "no more objects" received and triangulation is complete. 

Guess a width for line objects. 
Add boundary - fill while triangle intersects line rectangle.

# Vegetation 

Assign vegetation points to triangles, just as for water points. Many points - this will likely take some time. It can be started as soon as the triangulation completes.

The result is a triangle per vegetation point. This can be done in parallel. Need to divide list manually, since we want to remember result of last iteration. 

let veg_points = records.iter().map(record_to_point3d).collect();
let heights_for_veg_points.

let matching_veg_points = 

rayon crate.

For each triangle, get statistics (= number of points, top height). This can also be done in parallel.
Smooting filter based on the 3+6 adjacent triangles. This can also be done in parallel.

Growing - find yellow (< 1m height), green (large number of points, 2-4 m max height)

If there are a number of islands in yellow - halvöppen mark.

Density in each triangle will have


Calculate density, top height for each triangle. Triangles are too small for adequate statistics?

Assume that low top height correlates to high density. Difficult to tell if land has been "röjt" or not - top height will be the same. Density of ground points will be different - median triangle area larger for dense forest.

Can say that height below 5 m will be green. Height < 0.5 m should be yellow.

Grow area as long as we are within 1 sigma of height and average triangle size.

Exclusion list: remove intersections with residential land, and meadows.


# Contours

add contour structs which have references to above (can be more than one) and below (can be 0-1).

Find super-contour: move down. The first contour triangle that we encounter is the super-contour. We may reach the edge of the map. In this case we must continue along the edge until we reach ourselves (at both points) or our starting point. Or we just give up. 

Depth-first pathfinding.

# Status 2020-04-28

Still far away from target status as mapmaker.

- Subpar performance on old Lantmäteriet files. 
- A lot of work remains on the drone software. 
- New Laserdata Skog with higher point density is coming.

1. Start tuning on newer LAS files. Files for Gothenburg were recently released - there might be areas which overlap with maps that I have access to.
2. Start working on the carry rig for the LIDAR. Goal is to create LAS files before July 1st.
3. Software to walk around in the DTM. This will be needed either way, but otoh will be yet another new project started before other parts are finished.

Option 1. Check which files are available.

