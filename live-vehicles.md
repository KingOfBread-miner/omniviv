-   When asked for vehicles on a route the api should return a list of vehicles for the specified route with all their past and future departures and arrivals. from this list the position is calculated by the client with the current time, the list is updated every few seconds to ensure we have the latest realtime data from the api including delays

-   Smooth animations on the routes between stop positions realized in the frontend
-   Use client time to pinpoint the location on the route
-   Vehicles must move on the route geometry between stop positions not just the direct connection

-   Stop at stop positions using arrival and departure times
-   generate unique ids ourselves because we cant rely on unique ids from the providers

-   vehicles should never just vanish on a route if they arent put out of service
-   vehicles should accelerate and decelerate before and after stop positions
