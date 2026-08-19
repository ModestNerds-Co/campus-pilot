#
#  campus-pilot
#  Dockerfile
#
#  Created by Ngonidzashe Mangudya on 21/08/2025.
#  Copyright (c) 2025 Codecraft Solutions
#

# Production build stage
FROM node:18-alpine as builder

# Install pnpm
RUN npm install -g pnpm

# Set working directory
WORKDIR /app

# Copy package files
COPY package.json pnpm-lock.yaml ./

# Install dependencies
RUN pnpm install

# Copy source code
COPY . .

# Build the frontend
RUN pnpm run build

# Production stage
FROM node:18-alpine as production

# Install pnpm
RUN npm install -g pnpm

# Set working directory
WORKDIR /app

# Copy package files
COPY package.json pnpm-lock.yaml ./

# Install only production dependencies
RUN pnpm install --prod

# Copy built application from builder stage
COPY --from=builder /app/dist ./dist
