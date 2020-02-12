# Contours
So, the contours didn't work very well. 

From a mathematical contour `struct Contour` we need to define the points to use (or even Bezier). 

Then use Simplify in the geo package. This requires moving to Coordinate / geo structs. Keep Point3D for the DTM.


Remove over-detail: take a window of 9 points. 
Calculate average distance between points (this can be a property in the dtm). If the distance between the extreme points is short enough, remove all intermediate points.


Attempt to use flo_curves crate. fit_curve och fit_curve_cubic borde kunna användas för att ta fram Beziers. 48 punkter i taget - ger en reduktion med en faktor 4/48 = 1/12. 30 MB blir då ungefär 2,5 MB. 

Identify a stretch with sufficient x-y distance between 3 points. If no such stretch exists, then remove the contour completely. 

Get the vector from the last point.

Continue with points from true contour. Project onto vector. If distance is small enough (2 m?) keep going.

If 2-3 consecutive points all are on the same side



For the next point, continue along the same vector ? m. Check distance to true contour. If clo


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



