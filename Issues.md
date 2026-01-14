# End of line visualization

We have to create a clever way to handle the display of trams at beginning/end of routes, normally trams/vehicles move around in a loop, our model of the world ends the route so the model disappears at the end of the route and spawns a new one at the other route, this is okay in principal but we need to make sure the user does not notice this, so the vehicle needs to stop at the end of the route and the next vehicle at the start of the route needs to spawn in the same location at the same time as the old one vanishes, the tracking must also continue to the new vehicle

# Better Map design

-   Better colors
-   Dark mode
-   Time of day
-   transparent buildings with vehicles being visible through them or non transparent buildings, currently buildings are transparent but the vehicles are not visible through them

# Real vehicle models depending on zoom

# Clicking cant select two targets

clicking on vehicles does sometimes send the click through to a platform near it opening both the tracking of the vehicle as well as the platform departure monitor

# geo location tracking of the user

-   showing next departures with actual foot traffic timing

# route planner

# overlapping vehicles

Sometimes vehicles are overlapping due to similar timings, of course they cant overlap in real life so we need to adjust here too, also good for interface purposes, the vehicle that is minimal behind on the geometry should slow down so that its 3d model and/or indicator wont overlap with the other vehicles, if 2 vehicles have exactly the same timing the one with the higher ascii index or line number should slow down. It may be necessary that they intersect for a short while, while overtaking although not possible in real life fro trams this may be needed to keep their timing at stations. The goal here is to prevent driving in each other on the same line MOST OF THE TIME.

this is theoretically there but needs improvement

# station departure monitors should show departure in minutes and seconds

# traffic light for certain stations/platforms when to leave the house to be at the platform at the perfect time

# 3d terrain

-   including tube lines that are actually running underground
