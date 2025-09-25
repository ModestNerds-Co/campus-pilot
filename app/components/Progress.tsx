import * as React from "react"
import { cn } from "../lib/utils"

export interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number
  max?: number
  showPercentage?: boolean
  size?: 'sm' | 'default' | 'lg'
  variant?: 'default' | 'success' | 'warning' | 'error'
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({
    className,
    value = 0,
    max = 100,
    showPercentage = false,
    size = 'default',
    variant = 'default',
    ...props
  }, ref): React.ReactElement => {
    const percentage = Math.min(Math.max((value / max) * 100, 0), 100)

    const sizeClasses = {
      sm: "h-1.5",
      default: "h-2.5",
      lg: "h-4"
    }

    const variantClasses = {
      default: "bg-blue-600",
      success: "bg-green-600",
      warning: "bg-yellow-600",
      error: "bg-red-600"
    }

    return React.createElement(
      "div",
      {
        ref,
        className: cn("relative w-full overflow-hidden rounded-full bg-gray-200", sizeClasses[size], className),
        ...props
      },
      React.createElement(
        "div",
        {
          className: cn(
            "h-full w-full flex-1 transition-all duration-200 ease-in-out",
            variantClasses[variant]
          ),
          style: {
            transform: `translateX(-${100 - percentage}%)`
          }
        }
      ),
      showPercentage && React.createElement(
        "div",
        {
          className: "absolute inset-0 flex items-center justify-center text-xs font-medium text-gray-700"
        },
        `${Math.round(percentage)}%`
      )
    )
  }
)
Progress.displayName = "Progress"

export { Progress }
