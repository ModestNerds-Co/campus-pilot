import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Search, X } from "lucide-react";

import { cn } from "../lib/utils";

type SearchableSelectValue = string | number;

interface Option<T extends SearchableSelectValue> {
  id: T;
  value: string;
  label: string;
  description?: string;
}

interface SearchableSelectProps<T extends SearchableSelectValue> {
  id?: string;
  options: Option<T>[];
  value?: T | null;
  onChange: (value: T | null) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  loading?: boolean;
  allowClear?: boolean;
}

export function SearchableSelect<T extends SearchableSelectValue = number>({
  id,
  options,
  value,
  onChange,
  placeholder = "Select option...",
  disabled = false,
  className,
  loading = false,
  allowClear = true,
}: SearchableSelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const generatedId = useId().replace(/:/g, "");
  const listboxId = `searchable-select-${generatedId}`;

  const filteredOptions = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return options;
    return options.filter(
      (option) =>
        option.label.toLowerCase().includes(query) ||
        option.value.toLowerCase().includes(query) ||
        option.description?.toLowerCase().includes(query),
    );
  }, [options, searchQuery]);

  const visibleValues = useMemo<Array<T | null>>(
    () => [...(allowClear ? [null] : []), ...filteredOptions.map((option) => option.id)],
    [allowClear, filteredOptions],
  );

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
    if (!isOpen) return;
    const selectedIndex = visibleValues.findIndex((optionValue) => optionValue === value);
    setActiveIndex(Math.max(0, selectedIndex));
    searchInputRef.current?.focus();
  }, [isOpen, value, visibleValues]);

  useEffect(() => {
    setActiveIndex((current) => Math.min(current, Math.max(visibleValues.length - 1, 0)));
  }, [visibleValues.length]);

  const selectedOption = options.find((option) => option.id === value);

  const closeDropdown = (restoreFocus = false) => {
    setIsOpen(false);
    setSearchQuery("");
    if (restoreFocus) window.requestAnimationFrame(() => triggerRef.current?.focus());
  };

  const handleSelect = (optionValue: T | null) => {
    onChange(optionValue);
    closeDropdown(true);
  };

  const openDropdown = () => {
    if (disabled || loading) return;
    setSearchQuery("");
    setIsOpen(true);
  };

  const handleSearchKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeDropdown(true);
      return;
    }
    if (visibleValues.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((current) => (current + 1) % visibleValues.length);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((current) => (current - 1 + visibleValues.length) % visibleValues.length);
    } else if (event.key === "Home") {
      event.preventDefault();
      setActiveIndex(0);
    } else if (event.key === "End") {
      event.preventDefault();
      setActiveIndex(visibleValues.length - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      handleSelect(visibleValues[activeIndex]);
    }
  };

  const activeValue = visibleValues[activeIndex];
  const activeDescendant = isOpen && visibleValues.length > 0
    ? `${listboxId}-option-${activeValue ?? "clear"}`
    : undefined;

  return (
    <div className={cn("relative", className)} ref={dropdownRef}>
      <button
        aria-controls={isOpen ? listboxId : undefined}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        className={cn(
          "flex h-[var(--h-control-md)] w-full items-center justify-between rounded-[var(--radius-md)] border bg-[var(--surface)] py-2 pl-3 text-left text-sm text-[var(--text-strong)] shadow-sm",
          selectedOption && allowClear && !disabled ? "pr-16" : "pr-3",
          "border-[var(--border)] hover:border-[var(--border-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]",
          disabled && "cursor-not-allowed bg-[var(--surface-muted)] text-[var(--text-subtle)] opacity-60",
          isOpen && "border-[var(--focus-ring)] ring-2 ring-[var(--focus-ring)]",
        )}
        disabled={disabled || loading}
        id={id}
        onClick={() => (isOpen ? closeDropdown() : openDropdown())}
        ref={triggerRef}
        type="button"
      >
        <span className={cn("block truncate", !selectedOption && "text-[var(--text-subtle)]")}>
          {loading ? (
            "Loading..."
          ) : selectedOption ? (
            <span>
              <span className="font-medium">{selectedOption.value}</span>
              {selectedOption.description ? (
                <span className="ml-2 text-[var(--text-muted)]">({selectedOption.description})</span>
              ) : null}
            </span>
          ) : (
            placeholder
          )}
        </span>
        <ChevronDown className={cn("size-4 shrink-0 text-[var(--text-muted)] transition-transform", isOpen && "rotate-180")} />
      </button>

      {selectedOption && allowClear && !disabled ? (
        <button
          aria-label="Clear selection"
          className="absolute right-8 top-1/2 inline-flex size-8 -translate-y-1/2 items-center justify-center rounded-[var(--radius-sm)] text-[var(--text-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--text-strong)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]"
          onClick={() => handleSelect(null)}
          type="button"
        >
          <X className="size-3.5" />
        </button>
      ) : null}

      {isOpen ? (
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-[var(--radius-lg)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-popover)]">
          <div className="border-b border-[var(--border)] p-2">
            <div className="relative">
              <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--text-muted)]" />
              <input
                aria-activedescendant={activeDescendant}
                aria-autocomplete="list"
                aria-controls={listboxId}
                aria-expanded="true"
                aria-label="Search options"
                className="w-full rounded-[var(--radius-md)] border border-[var(--border)] bg-[var(--surface)] py-2 pl-9 pr-3 text-sm text-[var(--text-strong)] placeholder:text-[var(--text-subtle)] focus:outline-none focus:ring-2 focus:ring-[var(--focus-ring)]"
                onChange={(event) => setSearchQuery(event.target.value)}
                onKeyDown={handleSearchKeyDown}
                placeholder="Search options..."
                ref={searchInputRef}
                role="combobox"
                type="text"
                value={searchQuery}
              />
            </div>
          </div>

          <div className="max-h-60 overflow-y-auto" id={listboxId} role="listbox">
            {allowClear ? (
              <button
                aria-selected={value == null}
                className={cn(
                  "flex w-full items-center justify-between px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)]",
                  activeValue === null && "bg-[var(--surface-muted)]",
                  value == null && "text-[var(--brand-strong)]",
                )}
                id={`${listboxId}-option-clear`}
                onClick={() => handleSelect(null)}
                onMouseEnter={() => setActiveIndex(0)}
                role="option"
                tabIndex={-1}
                type="button"
              >
                <span className="italic text-[var(--text-muted)]">Clear selection</span>
                {value == null ? <Check className="size-4" /> : null}
              </button>
            ) : null}

            {filteredOptions.length === 0 ? (
              <div className="px-3 py-2 text-sm italic text-[var(--text-muted)]" role="status">No options found</div>
            ) : (
              filteredOptions.map((option, optionIndex) => {
                const index = optionIndex + (allowClear ? 1 : 0);
                return (
                  <button
                    aria-selected={option.id === value}
                    className={cn(
                      "flex w-full items-center justify-between px-3 py-2 text-left text-sm hover:bg-[var(--surface-muted)]",
                      index === activeIndex && "bg-[var(--surface-muted)]",
                      option.id === value && "text-[var(--brand-strong)]",
                    )}
                    id={`${listboxId}-option-${option.id}`}
                    key={option.id}
                    onClick={() => handleSelect(option.id)}
                    onMouseEnter={() => setActiveIndex(index)}
                    role="option"
                    tabIndex={-1}
                    type="button"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block font-medium text-[var(--text-strong)]">{option.value}</span>
                      {option.label !== option.value ? (
                        <span className="block truncate text-xs text-[var(--text-muted)]">{option.label}</span>
                      ) : null}
                      {option.description ? (
                        <span className="block truncate text-xs text-[var(--text-subtle)]">{option.description}</span>
                      ) : null}
                    </span>
                    {option.id === value ? <Check className="size-4 shrink-0" /> : null}
                  </button>
                );
              })
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
