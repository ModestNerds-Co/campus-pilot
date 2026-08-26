//
//  campus-pilot
//  index.ts - Fleet Management Module Exports
//
//  Created by Ngonidzashe Mangudya on 21/08/2026.
//  Copyright (c) 2025 Codecraft Solutions
//

export type * from "./types";

export { vehiclesService } from "./services/vehicles-service";
export { driversService } from "./services/drivers-service";

export { VehiclesList } from "./components/vehicles-list";
export { DriversList } from "./components/drivers-list";
export { VehicleFormModal } from "./components/vehicle-form-modal";
export { DriverFormModal } from "./components/driver-form-modal";
