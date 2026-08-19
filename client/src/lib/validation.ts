//
//  campus-pilot
//  validation.ts - App-wide Validation Utilities
//
//  Created by Ngonidzashe Mangudya on 26/09/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

export interface ValidationResult {
  isValid: boolean;
  errors: string[];
}

export interface FieldValidation {
  isValid: boolean;
  error?: string;
}

// Email validation
export const validateEmail = (email: string): FieldValidation => {
  if (!email.trim()) {
    return { isValid: false, error: "Email is required" };
  }

  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  if (!emailRegex.test(email)) {
    return { isValid: false, error: "Please enter a valid email address" };
  }

  return { isValid: true };
};

// Phone validation (international format)
export const validatePhone = (phone: string): FieldValidation => {
  if (!phone.trim()) {
    return { isValid: true }; // Phone is optional in most contexts
  }

  const phoneRegex = /^[\+]?[1-9][\d]{0,15}$/;
  if (!phoneRegex.test(phone.replace(/[\s\-\(\)]/g, ""))) {
    return { isValid: false, error: "Please enter a valid phone number" };
  }

  return { isValid: true };
};

// Password strength validation
export interface PasswordStrength {
  score: 0 | 1 | 2 | 3 | 4; // weak, fair, good, strong, very strong
  feedback: string[];
  isValid: boolean;
  label: "Very Weak" | "Weak" | "Fair" | "Good" | "Strong";
}

export const validatePassword = (password: string): PasswordStrength => {
  const feedback: string[] = [];
  let score: PasswordStrength["score"] = 0;

  if (password.length < 10) {
    feedback.push("Use at least 10 characters");
  } else {
    score += 1;
  }

  if (!/\d/.test(password)) {
    feedback.push("Include at least one number");
  } else {
    score += 1;
  }

  if (!/[a-z]/.test(password)) {
    feedback.push("Include at least one lowercase letter");
  } else {
    score += 1;
  }

  if (!/[A-Z]/.test(password)) {
    feedback.push("Include at least one uppercase letter");
  } else {
    score += 1;
  }

  if (!/[!@#$%^&*(),.?":{}|<>]/.test(password)) {
    feedback.push("Include at least one symbol (!@#$%^&*)");
  } else {
    score += 1;
  }

  // Bonus points for longer passwords
  if (password.length >= 15) score += 1;
  if (password.length >= 20) score += 1;

  // Cap at 4
  score = Math.min(score, 4) as PasswordStrength["score"];

  const labels: PasswordStrength["label"][] = [
    "Very Weak",
    "Weak",
    "Fair",
    "Good",
    "Strong",
  ];

  return {
    score,
    feedback,
    isValid: score >= 3, // At least good strength required
    label: labels[score],
  };
};

// Image validation
export interface ImageValidation extends FieldValidation {
  size?: number;
  dimensions?: { width: number; height: number };
  warnings?: string[];
}

export const validateImage = async (
  file: File,
  options: {
    maxSize?: number;
    minDimensions?: { width: number; height: number };
    allowedTypes?: string[];
  } = {},
): Promise<ImageValidation> => {
  const {
    maxSize = 2 * 1024 * 1024, // 2MB default
    minDimensions = { width: 128, height: 128 },
    allowedTypes = ["image/png", "image/jpeg", "image/jpg", "image/svg+xml"],
  } = options;

  // Check file size
  if (file.size > maxSize) {
    return {
      isValid: false,
      error: `Image must be smaller than ${Math.round(maxSize / (1024 * 1024))}MB`,
      size: file.size,
    };
  }

  // Check file type
  if (!allowedTypes.includes(file.type)) {
    return {
      isValid: false,
      error: "Only PNG, JPG, and SVG images are allowed",
    };
  }

  // Check dimensions for raster images
  if (file.type !== "image/svg+xml") {
    try {
      const dimensions = await getImageDimensions(file);
      const warnings: string[] = [];

      if (
        dimensions.width < minDimensions.width ||
        dimensions.height < minDimensions.height
      ) {
        return {
          isValid: false,
          error: `Image must be at least ${minDimensions.width}x${minDimensions.height} pixels`,
          size: file.size,
          dimensions,
        };
      }

      // Warn about extreme aspect ratios but don't fail
      const aspectRatio = dimensions.width / dimensions.height;
      if (aspectRatio > 3 || aspectRatio < 0.33) {
        warnings.push(
          "Image has an unusual aspect ratio. Consider using a more square image.",
        );
      }

      return {
        isValid: true,
        size: file.size,
        dimensions,
        warnings: warnings.length > 0 ? warnings : undefined,
      };
    } catch {
      return {
        isValid: false,
        error: "Unable to read image file",
      };
    }
  }

  return {
    isValid: true,
    size: file.size,
  };
};

// Utility function to get image dimensions
export const getImageDimensions = (
  file: File,
): Promise<{ width: number; height: number }> => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve({ width: img.width, height: img.height });
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Failed to load image"));
    };

    img.src = url;
  });
};

// Convert file to base64
export const fileToBase64 = (file: File): Promise<string> => {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => {
      if (typeof reader.result === "string") {
        // Remove the data URL prefix to get just the base64 data
        const base64 = reader.result.split(",")[1];
        resolve(base64);
      } else {
        reject(new Error("Failed to read file"));
      }
    };
    reader.onerror = reject;
  });
};

// Check if Caps Lock is on (for password fields)
export const checkCapsLock = (event: React.KeyboardEvent): boolean => {
  return event.getModifierState && event.getModifierState("CapsLock");
};

// Generic form field validation
export const validateRequired = (
  value: any,
  fieldName: string,
): FieldValidation => {
  if (!value || (typeof value === "string" && !value.trim())) {
    return { isValid: false, error: `${fieldName} is required` };
  }
  return { isValid: true };
};

// String length validation
export const validateLength = (
  value: string,
  min: number,
  max: number,
  fieldName: string,
): FieldValidation => {
  const trimmed = value.trim();
  if (trimmed.length < min) {
    return {
      isValid: false,
      error: `${fieldName} must be at least ${min} characters`,
    };
  }
  if (trimmed.length > max) {
    return {
      isValid: false,
      error: `${fieldName} must be no more than ${max} characters`,
    };
  }
  return { isValid: true };
};
