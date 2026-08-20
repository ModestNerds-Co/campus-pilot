//
//  campus-pilot
//  SearchableSelect.tsx (token-driven)
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

  useEffect(() => {
    if (!searchQuery.trim()) {
      setFilteredOptions(options);
    } else {
      const filtered = options.filter(
        (option) =>
          option.label.toLowerCase().includes(searchQuery.toLowerCase()) ||
          option.value.toLowerCase().includes(searchQuery.toLowerCase()) ||
          (option.description && option.description.toLowerCase().includes(searchQuery.toLowerCase())),
      );
      setFilteredOptions(filtered);
    }
  }, [searchQuery, options]);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
        setSearchQuery("");
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  useEffect(() => {
    if (isOpen && searchInputRef.current) searchInputRef.current.focus();
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
      if (!isOpen) setSearchQuery("");
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
      <button
        type="button"
        onClick={handleToggle}
        disabled={disabled || loading}
        className={cn(
          "flex h-[var(--h-control-md)] w-full items-center justify-between rounded-[var(--radius-md)] border bg-[var(--surface)] px-3 py-2 text-left text-sm text-[var(--text-strong)] shadow-sm",
          "border-[var(--border)] hover:border-[var(--border-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]",
          disabled && "cursor-not-allowed bg-[var(--surface-muted)] text-[var(--text-subtle)] opacity-60",
          isOpen && "ring-2 ring-[var(--focus-ring)] border-[var(--focus-ring)]",
        )}
      >
        <span className={cn("block truncate", !selectedOption && "text-[var(--text-subtle)]")}>
          {loading ? (
            "Loading..."
          ) : selectedOption ? (
            <span>
              <span className="font-medium">{selectedOption.value}</span>
              {selectedOption.description && (
                <span className="ml-2 text-[var(--text-muted)]">({selectedOption.description})</span>
              )}
            </span>
          ) : (
            placeholder
          )}
        </span>

        <div className="flex items-center gap-1">
          {selectedOption && allowClear && !disabled && (
            <span
              role="button"
              tabIndex={0}
              onClick={(e) => {
                e.stopPropagation();
                handleSelect(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.stopPropagation();
                  handleSelect(null);
                }
              }}
              className="rounded p-1 text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)]"
              aria-label="Clear selection"
            >
              ×
            </span>
          )}
          <ChevronDown className={cn("size-4 text-[var(--text-muted)] transition-transform", isOpen && "rotate-180")} />
        </div>
      </button>

      {isOpen && (
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-popover)]">
          <div className="border-b border-[var(--border)] p-2">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--text-muted)]" />
              <input
                ref={searchInputRef}
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Search options..."
                className="w-full rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] py-2 pl-9 pr-3 text-sm text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus:outline-none focus:ring-2 focus:ring-[var(--focus-ring)]"
              />
            </div>
          </div>

          <div className="max-h-60 overflow-y-auto">
            {allowClear && (
              <button
                type="button"
                onClick={() => handleSelect(null)}
                className={cn(
                  "flex w-full items-center justify-between px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)]",
                  value === null && "bg-[var(--brand-soft)] text-[var(--brand-strong)]",
                )}
              >
                <span className="italic text-[var(--text-muted)]">Clear selection</span>
                {value === null && <Check className="size-4" />}
              </button>
            )}

            {filteredOptions.length === 0 ? (
              <div className="px-3 py-2 text-sm italic text-[var(--text-muted)]">No options found</div>
            ) : (
              filteredOptions.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  onClick={() => handleSelect(option.id)}
                  className={cn(
                    "flex w-full items-center justify-between px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)]",
                    option.id === value && "bg-[var(--brand-soft)] text-[var(--brand-strong)]",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="font-medium text-[var(--text-strong)]">{option.value}</div>
                    {option.label !== option.value && (
                      <div className="truncate text-xs text-[var(--text-muted)]">{option.label}</div>
                    )}
                    {option.description && (
                      <div className="truncate text-xs text-[var(--text-subtle)]">{option.description}</div>
                    )}
                  </div>
                  {option.id === value && <Check className="size-4 shrink-0" />}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
