//
//  campus-pilot
//  db.ts
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import pg from "pg";
import dotenv from "dotenv";

dotenv.config();

const { Pool } = pg;

// Database connection configuration
const dbConfig = {
  host: process.env.DB_HOST || "localhost",
  port: parseInt(process.env.DB_PORT || "5432"),
  database: process.env.DB_NAME || "immigration_db",
  user: process.env.DB_USER || "postgres",
  password: process.env.DB_PASSWORD || "",
  max: 20, // Maximum number of clients in the pool
  idleTimeoutMillis: 30000,
  connectionTimeoutMillis: 5000,
};

// Create a connection pool
export const pool = new Pool(dbConfig);

// Connection status tracking
let isConnected = false;
let connectionError: Error | null = null;

// Test database connection
export async function testConnection(): Promise<{
  success: boolean;
  error?: string;
}> {
  try {
    const client = await pool.connect();
    await client.query("SELECT NOW()");
    client.release();
    isConnected = true;
    connectionError = null;
    return { success: true };
  } catch (error) {
    isConnected = false;
    connectionError = error as Error;
    console.error("Database connection failed:", error);
    return {
      success: false,
      error:
        error instanceof Error ? error.message : "Unknown connection error",
    };
  }
}

// Get connection status
export function getConnectionStatus() {
  return {
    isConnected,
    error: connectionError?.message || null,
  };
}

// Execute a query with automatic retry
export async function query<T extends pg.QueryResultRow = any>(
  text: string,
  params?: any[],
): Promise<pg.QueryResult<T>> {
  try {
    const result = await pool.query<T>(text, params);
    isConnected = true;
    connectionError = null;
    return result;
  } catch (error) {
    isConnected = false;
    connectionError = error as Error;
    throw error;
  }
}

// Transaction helper
export async function withTransaction<T>(
  callback: (client: pg.PoolClient) => Promise<T>,
): Promise<T> {
  const client = await pool.connect();
  try {
    await client.query("BEGIN");
    const result = await callback(client);
    await client.query("COMMIT");
    return result;
  } catch (error) {
    await client.query("ROLLBACK");
    throw error;
  } finally {
    client.release();
  }
}

// Cleanup on process termination
process.on("SIGINT", async () => {
  await pool.end();
  process.exit(0);
});

process.on("SIGTERM", async () => {
  await pool.end();
  process.exit(0);
});
