//
//  campus-pilot
//  SearchableSelect.tsx
//
//  Created by Ngonidzashe Mangudya on 21/08/2025.
//  Copyright (c) 2025 Codecraft Solutions
//

import React, { useState, useEffect, useRef } from "react";
import { ChevronDown, Search, Check } from "lucide-react";
import { cn } from "../lib/utils";

interface Option {
  id: number;
  value: string;
  label: string;
  description?: string;
}

interface SearchableSelectProps {
  options: Option[];
  value?: number | null;
  onChange: (value: number | null) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  loading?: boolean;
  allowClear?: boolean;
}

export function SearchableSelect({
  options,
  value,
  onChange,
  placeholder = "Select option...",
  disabled = false,
  className,
  loading = false,
  allowClear = true,
}: SearchableSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [filteredOptions, setFilteredOptions] = useState<Option[]>(options);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);

  // Filter options based on search query
  useEffect(() => {
    if (!searchQuery.trim()) {
      setFilteredOptions(options);
    } else {
      const filtered = options.filter(
        (option) =>
          option.label.toLowerCase().includes(searchQuery.toLowerCase()) ||
          option.value.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (option.description &&
            option.description
              .toLowerCase()
              .includes(searchQuery.toLowerCase())),
      );
      setFilteredOptions(filtered);
    }
  }, [searchQuery, options]);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
        setSearchQuery("");
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Focus search input when dropdown opens
  useEffect(() => {
    if (isOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isOpen]);

  const selectedOption = options.find((option) => option.id === value);

  const handleSelect = (optionValue: number | null) => {
    onChange(optionValue);
    setIsOpen(false);
    setSearchQuery("");
  };

  const handleToggle = () => {
    if (!disabled) {
      setIsOpen(!isOpen);
      if (!isOpen) {
        setSearchQuery("");
      }
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setIsOpen(false);
      setSearchQuery("");
    } else if (e.key === "Enter" && filteredOptions.length === 1) {
      handleSelect(filteredOptions[0].id);
    }
  };

  return (
    <div className={cn("relative", className)} ref={dropdownRef}>
      {/* Trigger Button */}
      <button
        type="button"
        onClick={handleToggle}
        disabled={disabled || loading}
        className={cn(
          "w-full flex items-center justify-between px-4 py-3 text-left bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-xl shadow-sm text-sm dark:text-white",
          "hover:border-gray-400 dark:hover:border-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500",
          disabled &&
            "bg-gray-50 dark:bg-gray-800 text-gray-500 dark:text-gray-400 cursor-not-allowed",
          isOpen && "ring-2 ring-blue-500 border-blue-500",
        )}
      >
        <span
          className={cn(
            "block truncate",
            !selectedOption && "text-gray-500 dark:text-gray-400",
          )}
        >
          {loading ? (
            "Loading..."
          ) : selectedOption ? (
            <span>
              <span className="font-medium">{selectedOption.value}</span>
              {selectedOption.description && (
                <span className="text-gray-500 dark:text-gray-400 ml-2">
                  ({selectedOption.description})
                </span>
              )}
            </span>
          ) : (
            placeholder
          )}
        </span>

        <div className="flex items-center gap-1">
          {selectedOption && allowClear && !disabled && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleSelect(null);
              }}
              className="p-1 hover:bg-gray-100 dark:hover:bg-gray-600 rounded"
            >
              <span className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">
                ×
              </span>
            </button>
          )}
          <ChevronDown
            className={cn(
              "w-4 h-4 text-gray-400 dark:text-gray-500 transition-transform",
              isOpen && "transform rotate-180",
            )}
          />
        </div>
      </button>

      {/* Dropdown */}
      {isOpen && (
        <div className="absolute z-50 w-full mt-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-xl shadow-lg">
          {/* Search Input */}
          <div className="p-2 border-b border-gray-200 dark:border-gray-700">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-gray-400 dark:text-gray-500" />
              <input
                ref={searchInputRef}
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Search options..."
                className="w-full pl-9 pr-3 py-2 text-sm border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded-lg focus:outline-none focus:ring-1 focus:ring-blue-500 focus:border-blue-500"
              />
            </div>
          </div>

          {/* Options List */}
          <div className="max-h-60 overflow-y-auto">
            {allowClear && (
              <button
                type="button"
                onClick={() => handleSelect(null)}
                className={cn(
                  "w-full px-3 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-gray-700 flex items-center justify-between",
                  value === null &&
                    "bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300",
                )}
              >
                <span className="text-gray-500 dark:text-gray-400 italic">
                  Clear selection
                </span>
                {value === null && <Check className="w-4 h-4" />}
              </button>
            )}

            {filteredOptions.length === 0 ? (
              <div className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400 italic">
                No options found
              </div>
            ) : (
              filteredOptions.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  onClick={() => handleSelect(option.id)}
                  className={cn(
                    "w-full px-3 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-gray-700 flex items-center justify-between",
                    option.id === value &&
                      "bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-300",
                  )}
                >
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-gray-900 dark:text-white">
                      {option.value}
                    </div>
                    {option.label !== option.value && (
                      <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
                        {option.label}
                      </div>
                    )}
                    {option.description && (
                      <div className="text-xs text-gray-400 dark:text-gray-500 truncate">
                        {option.description}
                      </div>
                    )}
                  </div>
                  {option.id === value && (
                    <Check className="w-4 h-4 flex-shrink-0" />
                  )}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
