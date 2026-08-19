// TODO: Fix TanStack Start createServerFn API usage
// import { createServerFn } from "@tanstack/start";
import { testConnection, getConnectionStatus } from "../db";

// Test database connection
export const testDatabaseConnection = async () => {
  const result = await testConnection();
  return result;
};

// Get current connection status
export const getDatabaseConnectionStatus = async () => {
  return getConnectionStatus();
};

// Health check endpoint
export const healthCheck = async () => {
  const dbStatus = getConnectionStatus();
  const timestamp = new Date().toISOString();

  return {
    status: dbStatus.isConnected ? "healthy" : "unhealthy",
    timestamp,
    database: {
      connected: dbStatus.isConnected,
      error: dbStatus.error,
    },
    version: "1.0.0",
  };
};
