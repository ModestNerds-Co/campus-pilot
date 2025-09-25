//
//  campus-pilot
//  Select.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import * as React from "react";
import { cn } from "../lib/utils";

export interface SelectProps
  extends React.SelectHTMLAttributes<HTMLSelectElement> {
  placeholder?: string;
}

const Select = React.forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, placeholder, children, ...props }, ref): React.ReactElement => {
    return React.createElement(
      "select",
      {
        className: cn(
          "flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm ring-offset-background placeholder:text-gray-500 focus:border-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
          className,
        ),
        ref,
        ...props,
      },
      placeholder &&
        React.createElement(
          "option",
          { value: "", disabled: true },
          placeholder,
        ),
      children,
    );
  },
);
Select.displayName = "Select";

const SelectItem = React.forwardRef<
  HTMLOptionElement,
  React.OptionHTMLAttributes<HTMLOptionElement>
>(({ className, ...props }, ref): React.ReactElement => {
  return React.createElement("option", {
    ref: ref,
    className: cn("relative cursor-default select-none py-1.5 px-2", className),
    ...props,
  });
});
SelectItem.displayName = "SelectItem";

export { Select, SelectItem };
