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


- Generate 0,5 m contours
- Calculate contour scores.
- 10 different - create a thread for each one. Post (z_offset, Vec<Contour>) to collator
- Decide overall 5 m interval. 1 out of 10.
- Deviate 0,5 - 1 m from global
- Locally deviate up to 1,5 m for a part of the contour (optional)
- Replace small contours with knolls
- Look 1,5-4 m up to find small closed curves and add hjälpkurvor for those
- Select one 5 m level for stödkurvor symbol.

Also need a better (or more tailored) Bezier fitting algoritm, that doesn't introduce artifacts. Is there a way to identify sänkor / näsor - these need to be control points.

# General boundary improvements

Keep track of the total area. This way we can enforce a minimum requirement on area.

# Water model

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

# Vegetation 

Assign vegetation points to triangles, just as for water points.

Calculate density, top height for each triangle. Triangles are too small for adequate statistics?

Assume that low top height correlates to high density. Difficult to tell if land has been "röjt" or not - top height will be the same. Density of ground points will be different - median triangle area larger for dense forest.

Can say that height below 5 m will be green. Height < 0.5 m should be yellow.

Grow area as long as we are within 1 sigma of height and average triangle size.




Exclusion list: remove intersections with residential land, and meadows.



