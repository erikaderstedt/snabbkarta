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
