//
//  campus-pilot
//  Spinner.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import * as React from "react";
import { cn } from "../lib/utils";

export interface SpinnerProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: "sm" | "default" | "lg";
  variant?: "default" | "primary" | "secondary";
}

const Spinner = React.forwardRef<HTMLDivElement, SpinnerProps>(
  (
    { className, size = "default", variant = "default", ...props },
    ref,
  ): React.ReactElement => {
    const sizeClasses = {
      sm: "w-4 h-4",
      default: "w-6 h-6",
      lg: "w-8 h-8",
    };

    const variantClasses = {
      default: "border-gray-300 border-t-gray-600",
      primary: "border-blue-200 border-t-blue-600",
      secondary: "border-gray-200 border-t-gray-500",
    };

    return React.createElement("div", {
      ref,
      className: cn(
        "animate-spin rounded-full border-2",
        sizeClasses[size],
        variantClasses[variant],
        className,
      ),
      ...props,
    });
  },
);
Spinner.displayName = "Spinner";

export { Spinner };
