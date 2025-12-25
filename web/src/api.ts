/* eslint-disable */
/* tslint:disable */
/*
 * ---------------------------------------------------------------
 * ## THIS FILE WAS GENERATED VIA SWAGGER-TYPESCRIPT-API        ##
 * ##                                                           ##
 * ## AUTHOR: acacode                                           ##
 * ## SOURCE: https://github.com/acacode/swagger-typescript-api ##
 * ---------------------------------------------------------------
 */

export interface Area {
  created_at: string;
  /** @format double */
  east: number;
  /** @format int64 */
  id: number;
  last_synced_at?: string | null;
  name: string;
  /** @format double */
  north: number;
  /** @format double */
  south: number;
  /** @format double */
  west: number;
}

export interface AreaListResponse {
  areas: Area[];
}

export interface AreaStats {
  /** @format int64 */
  area_id: number;
  area_name: string;
  /** @format int64 */
  platform_count: number;
  /** @format int64 */
  route_count: number;
  /** @format int64 */
  station_count: number;
  /** @format int64 */
  stop_position_count: number;
}

/** A stop event (departure or arrival) */
export interface Departure {
  /** @format int32 */
  delay_minutes?: number | null;
  /** For departures: destination; for arrivals: origin */
  destination: string;
  /** Destination stop ID (for departures) or origin stop ID (for arrivals) */
  destination_id?: string | null;
  estimated_time?: string | null;
  /** Type of stop event */
  event_type: EventType;
  line_number: string;
  planned_time: string;
  platform?: string | null;
  stop_ifopt: string;
  /** Unique trip identifier (AVMSTripID) - consistent across all stops for a journey */
  trip_id?: string | null;
}

export interface DepartureListResponse {
  departures: Departure[];
}

export interface ErrorResponse {
  error: string;
}

/** Type of stop event */
export enum EventType {
  Departure = "departure",
  Arrival = "arrival",
}

export interface Route {
  /** @format int64 */
  area_id?: number | null;
  color?: string | null;
  name?: string | null;
  network?: string | null;
  operator?: string | null;
  /** @format int64 */
  osm_id: number;
  osm_type: string;
  ref?: string | null;
  route_type: string;
}

export type RouteDetail = Route & {
  stops: RouteStop[];
};

export interface RouteGeometry {
  /** @format int64 */
  route_id: number;
  segments: number[][][];
}

export interface RouteListResponse {
  routes: Route[];
}

export interface RouteStop {
  /** @format int64 */
  platform_id?: number | null;
  role?: string | null;
  /** @format int64 */
  sequence: number;
  /** @format int64 */
  station_id?: number | null;
  station_name?: string | null;
  /** @format int64 */
  stop_position_id?: number | null;
}

export interface Station {
  /** @format int64 */
  area_id?: number | null;
  /** @format double */
  lat: number;
  /** @format double */
  lon: number;
  name?: string | null;
  /** @format int64 */
  osm_id: number;
  osm_type: string;
  platforms: StationPlatform[];
  ref_ifopt?: string | null;
  stop_positions: StationStopPosition[];
}

export interface StationListResponse {
  stations: Station[];
}

/** Platform info nested in station response */
export interface StationPlatform {
  /** @format double */
  lat: number;
  /** @format double */
  lon: number;
  name?: string | null;
  /** @format int64 */
  osm_id: number;
  ref?: string | null;
  ref_ifopt?: string | null;
}

/** Stop position info nested in station response */
export interface StationStopPosition {
  /** @format double */
  lat: number;
  /** @format double */
  lon: number;
  name?: string | null;
  /** @format int64 */
  osm_id: number;
  /** @format int64 */
  platform_id?: number | null;
  ref?: string | null;
  ref_ifopt?: string | null;
}

export interface StopDeparturesRequest {
  stop_ifopt: string;
}

export interface StopDeparturesResponse {
  departures: Departure[];
  stop_ifopt: string;
}

export interface Vehicle {
  /** Final destination of this vehicle */
  destination: string;
  /** Line number (e.g., "1", "2", "3") */
  line_number: string;
  /** Origin of this vehicle's journey */
  origin?: string | null;
  /** All stops this vehicle will visit, in order */
  stops: VehicleStop[];
  /** Unique trip identifier (AVMSTripID from EFA) */
  trip_id: string;
}

export interface VehicleStop {
  /** Arrival time at this stop (ISO 8601) */
  arrival_time?: string | null;
  /** Estimated arrival time (real-time, if available) */
  arrival_time_estimated?: string | null;
  /**
   * Delay in minutes (positive = late, negative = early)
   * @format int32
   */
  delay_minutes?: number | null;
  /** Departure time from this stop (ISO 8601) */
  departure_time?: string | null;
  /** Estimated departure time (real-time, if available) */
  departure_time_estimated?: string | null;
  /**
   * Latitude
   * @format double
   */
  lat: number;
  /**
   * Longitude
   * @format double
   */
  lon: number;
  /**
   * Sequence number on the route
   * @format int64
   */
  sequence: number;
  /** Stop IFOPT identifier */
  stop_ifopt: string;
  /** Stop name (if available) */
  stop_name?: string | null;
}

export interface VehiclesByRouteRequest {
  /**
   * The OSM route ID to get vehicles for
   * @format int64
   */
  route_id: number;
}

export interface VehiclesByRouteResponse {
  line_number?: string | null;
  /** @format int64 */
  route_id: number;
  vehicles: Vehicle[];
}

export type QueryParamsType = Record<string | number, any>;
export type ResponseFormat = keyof Omit<Body, "body" | "bodyUsed">;

export interface FullRequestParams extends Omit<RequestInit, "body"> {
  /** set parameter to `true` for call `securityWorker` for this request */
  secure?: boolean;
  /** request path */
  path: string;
  /** content type of request body */
  type?: ContentType;
  /** query params */
  query?: QueryParamsType;
  /** format of response (i.e. response.json() -> format: "json") */
  format?: ResponseFormat;
  /** request body */
  body?: unknown;
  /** base url */
  baseUrl?: string;
  /** request cancellation token */
  cancelToken?: CancelToken;
}

export type RequestParams = Omit<FullRequestParams, "body" | "method" | "query" | "path">;

export interface ApiConfig<SecurityDataType = unknown> {
  baseUrl?: string;
  baseApiParams?: Omit<RequestParams, "baseUrl" | "cancelToken" | "signal">;
  securityWorker?: (securityData: SecurityDataType | null) => Promise<RequestParams | void> | RequestParams | void;
  customFetch?: typeof fetch;
}

export interface HttpResponse<D extends unknown, E extends unknown = unknown> extends Response {
  data: D;
  error: E;
}

type CancelToken = Symbol | string | number;

export enum ContentType {
  Json = "application/json",
  FormData = "multipart/form-data",
  UrlEncoded = "application/x-www-form-urlencoded",
  Text = "text/plain",
}

export class HttpClient<SecurityDataType = unknown> {
  public baseUrl: string = "";
  private securityData: SecurityDataType | null = null;
  private securityWorker?: ApiConfig<SecurityDataType>["securityWorker"];
  private abortControllers = new Map<CancelToken, AbortController>();
  private customFetch = (...fetchParams: Parameters<typeof fetch>) => fetch(...fetchParams);

  private baseApiParams: RequestParams = {
    credentials: "same-origin",
    headers: {},
    redirect: "follow",
    referrerPolicy: "no-referrer",
  };

  constructor(apiConfig: ApiConfig<SecurityDataType> = {}) {
    Object.assign(this, apiConfig);
  }

  public setSecurityData = (data: SecurityDataType | null) => {
    this.securityData = data;
  };

  protected encodeQueryParam(key: string, value: any) {
    const encodedKey = encodeURIComponent(key);
    return `${encodedKey}=${encodeURIComponent(typeof value === "number" ? value : `${value}`)}`;
  }

  protected addQueryParam(query: QueryParamsType, key: string) {
    return this.encodeQueryParam(key, query[key]);
  }

  protected addArrayQueryParam(query: QueryParamsType, key: string) {
    const value = query[key];
    return value.map((v: any) => this.encodeQueryParam(key, v)).join("&");
  }

  protected toQueryString(rawQuery?: QueryParamsType): string {
    const query = rawQuery || {};
    const keys = Object.keys(query).filter((key) => "undefined" !== typeof query[key]);
    return keys
      .map((key) => (Array.isArray(query[key]) ? this.addArrayQueryParam(query, key) : this.addQueryParam(query, key)))
      .join("&");
  }

  protected addQueryParams(rawQuery?: QueryParamsType): string {
    const queryString = this.toQueryString(rawQuery);
    return queryString ? `?${queryString}` : "";
  }

  private contentFormatters: Record<ContentType, (input: any) => any> = {
    [ContentType.Json]: (input: any) =>
      input !== null && (typeof input === "object" || typeof input === "string") ? JSON.stringify(input) : input,
    [ContentType.Text]: (input: any) => (input !== null && typeof input !== "string" ? JSON.stringify(input) : input),
    [ContentType.FormData]: (input: any) =>
      Object.keys(input || {}).reduce((formData, key) => {
        const property = input[key];
        formData.append(
          key,
          property instanceof Blob
            ? property
            : typeof property === "object" && property !== null
              ? JSON.stringify(property)
              : `${property}`,
        );
        return formData;
      }, new FormData()),
    [ContentType.UrlEncoded]: (input: any) => this.toQueryString(input),
  };

  protected mergeRequestParams(params1: RequestParams, params2?: RequestParams): RequestParams {
    return {
      ...this.baseApiParams,
      ...params1,
      ...(params2 || {}),
      headers: {
        ...(this.baseApiParams.headers || {}),
        ...(params1.headers || {}),
        ...((params2 && params2.headers) || {}),
      },
    };
  }

  protected createAbortSignal = (cancelToken: CancelToken): AbortSignal | undefined => {
    if (this.abortControllers.has(cancelToken)) {
      const abortController = this.abortControllers.get(cancelToken);
      if (abortController) {
        return abortController.signal;
      }
      return void 0;
    }

    const abortController = new AbortController();
    this.abortControllers.set(cancelToken, abortController);
    return abortController.signal;
  };

  public abortRequest = (cancelToken: CancelToken) => {
    const abortController = this.abortControllers.get(cancelToken);

    if (abortController) {
      abortController.abort();
      this.abortControllers.delete(cancelToken);
    }
  };

  public request = async <T = any, E = any>({
    body,
    secure,
    path,
    type,
    query,
    format,
    baseUrl,
    cancelToken,
    ...params
  }: FullRequestParams): Promise<HttpResponse<T, E>> => {
    const secureParams =
      ((typeof secure === "boolean" ? secure : this.baseApiParams.secure) &&
        this.securityWorker &&
        (await this.securityWorker(this.securityData))) ||
      {};
    const requestParams = this.mergeRequestParams(params, secureParams);
    const queryString = query && this.toQueryString(query);
    const payloadFormatter = this.contentFormatters[type || ContentType.Json];
    const responseFormat = format || requestParams.format;

    return this.customFetch(`${baseUrl || this.baseUrl || ""}${path}${queryString ? `?${queryString}` : ""}`, {
      ...requestParams,
      headers: {
        ...(requestParams.headers || {}),
        ...(type && type !== ContentType.FormData ? { "Content-Type": type } : {}),
      },
      signal: (cancelToken ? this.createAbortSignal(cancelToken) : requestParams.signal) || null,
      body: typeof body === "undefined" || body === null ? null : payloadFormatter(body),
    }).then(async (response) => {
      const r = response.clone() as HttpResponse<T, E>;
      r.data = null as unknown as T;
      r.error = null as unknown as E;

      const data = !responseFormat
        ? r
        : await response[responseFormat]()
            .then((data) => {
              if (r.ok) {
                r.data = data;
              } else {
                r.error = data;
              }
              return r;
            })
            .catch((e) => {
              r.error = e;
              return r;
            });

      if (cancelToken) {
        this.abortControllers.delete(cancelToken);
      }

      if (!response.ok) throw data;
      return data;
    });
  };
}

/**
 * @title Live Tram API
 * @version 0.1.0
 * @license
 */
export class Api<SecurityDataType extends unknown> extends HttpClient<SecurityDataType> {
  api = {
    /**
     * No description
     *
     * @tags areas
     * @name ListAreas
     * @summary List all configured areas
     * @request GET:/api/areas
     */
    listAreas: (params: RequestParams = {}) =>
      this.request<AreaListResponse, ErrorResponse>({
        path: `/api/areas`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags areas
     * @name GetArea
     * @summary Get a specific area by ID
     * @request GET:/api/areas/{id}
     */
    getArea: (id: number, params: RequestParams = {}) =>
      this.request<Area, ErrorResponse>({
        path: `/api/areas/${id}`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags areas
     * @name GetAreaStats
     * @summary Get statistics for an area
     * @request GET:/api/areas/{id}/stats
     */
    getAreaStats: (id: number, params: RequestParams = {}) =>
      this.request<AreaStats, ErrorResponse>({
        path: `/api/areas/${id}/stats`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags departures
     * @name ListDepartures
     * @summary List all departures across all stops
     * @request GET:/api/departures
     */
    listDepartures: (params: RequestParams = {}) =>
      this.request<DepartureListResponse, ErrorResponse>({
        path: `/api/departures`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags departures
     * @name GetDeparturesByStop
     * @summary Get departures for a specific stop by IFOPT ID
     * @request POST:/api/departures/by-stop
     */
    getDeparturesByStop: (data: StopDeparturesRequest, params: RequestParams = {}) =>
      this.request<StopDeparturesResponse, ErrorResponse>({
        path: `/api/departures/by-stop`,
        method: "POST",
        body: data,
        type: ContentType.Json,
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags routes
     * @name ListRoutes
     * @summary List all routes, optionally filtered by area or type
     * @request GET:/api/routes
     */
    listRoutes: (
      query?: {
        /**
         * Filter by area ID
         * @format int64
         */
        area_id?: number | null;
        /** Filter by route type (e.g., "tram", "bus") */
        route_type?: string | null;
      },
      params: RequestParams = {},
    ) =>
      this.request<RouteListResponse, ErrorResponse>({
        path: `/api/routes`,
        method: "GET",
        query: query,
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags routes
     * @name GetRoute
     * @summary Get a single route with its stops
     * @request GET:/api/routes/{route_id}
     */
    getRoute: (routeId: number, params: RequestParams = {}) =>
      this.request<RouteDetail, ErrorResponse>({
        path: `/api/routes/${routeId}`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags routes
     * @name GetRouteGeometry
     * @summary Get the geometry of a route as line segments
     * @request GET:/api/routes/{route_id}/geometry
     */
    getRouteGeometry: (routeId: number, params: RequestParams = {}) =>
      this.request<RouteGeometry, ErrorResponse>({
        path: `/api/routes/${routeId}/geometry`,
        method: "GET",
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags stations
     * @name ListStations
     * @summary List all stations that have platforms linked to them, optionally filtered by area
     * @request GET:/api/stations
     */
    listStations: (
      query?: {
        /**
         * Filter by area ID
         * @format int64
         */
        area_id?: number | null;
      },
      params: RequestParams = {},
    ) =>
      this.request<StationListResponse, ErrorResponse>({
        path: `/api/stations`,
        method: "GET",
        query: query,
        format: "json",
        ...params,
      }),

    /**
     * No description
     *
     * @tags vehicles
     * @name GetVehiclesByRoute
     * @summary Get all vehicles currently on a route with their stop sequences
     * @request POST:/api/vehicles/by-route
     */
    getVehiclesByRoute: (data: VehiclesByRouteRequest, params: RequestParams = {}) =>
      this.request<VehiclesByRouteResponse, ErrorResponse>({
        path: `/api/vehicles/by-route`,
        method: "POST",
        body: data,
        type: ContentType.Json,
        format: "json",
        ...params,
      }),
  };
}
