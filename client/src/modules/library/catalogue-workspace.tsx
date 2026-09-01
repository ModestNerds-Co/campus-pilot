import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { BookOpen, Loader2, Plus, Search } from "lucide-react";
import toast from "react-hot-toast";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableControlsBar,
  TableControlsPagination,
  TableEmpty,
  TableError,
  TableLoading,
  TableScroll,
  TableWrap,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from "@/components/ui/data-table";
import {
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogShell,
} from "@/components/ui/dialog";
import { Input, Label, Select, Textarea } from "@/components/ui/input";
import { usePageChrome } from "@/modules/admin/layouts/page-chrome";
import { useAuthStore } from "@/stores/auth-store";

import { libraryService, responseMessage } from "./service";
import { libraryAccessProfile } from "./access";
import type {
  CopyCondition,
  CopyRecord,
  CopyStatus,
  CurrencyReference,
  LibraryReferenceData,
  TitleDetail,
  TitlePayload,
  TitleSummary,
} from "./types";
import { displayValue, optional, statusTone } from "./ui";

export function LibraryCatalogueWorkspace() {
  const permissions = useAuthStore((state) => state.user?.permissions ?? []);
  const canCreate = allowed(permissions, "library:create");
  const canEdit = allowed(permissions, "library:edit");
  const canRetire = allowed(permissions, "library:delete");
  const { canManageCatalogue } = libraryAccessProfile(permissions);
  const [titles, setTitles] = useState<TitleSummary[]>([]);
  const [references, setReferences] = useState<LibraryReferenceData | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [totalPages, setTotalPages] = useState(1);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await libraryService.titles({
        page,
        per_page: 25,
        search: search.trim() || undefined,
        status: status === "all" ? undefined : status,
      });
      if (!response.success || !response.data)
        throw new Error(
          responseMessage(response, "Catalogue could not be loaded"),
        );
      setTitles(response.data.titles);
      setTotalPages(response.pagination?.total_pages ?? 1);
    } catch (loadError) {
      setError(
        loadError instanceof Error
          ? loadError.message
          : "Catalogue could not be loaded",
      );
    } finally {
      setLoading(false);
    }
  }, [page, search, status]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    if (!canCreate && !canEdit) return;
    void libraryService.references().then((response) => {
      if (response.success) setReferences(response.data ?? null);
    });
  }, [canCreate, canEdit]);
  usePageChrome(
    "Catalogue",
    canCreate ? (
      <Button
        onClick={() => {
          setSelectedId(null);
          setDrawerOpen(true);
        }}
      >
        <Plus className="size-4" />
        New title
      </Button>
    ) : null,
  );
  const filtered = Boolean(search.trim() || status !== "all");

  return (
    <div className="space-y-6">
      <p className="text-sm text-[var(--text-muted)]">
        {canManageCatalogue ? "Manage titles and physical copies." : "Search titles and available copies."}
      </p>
      <TableControlsBar>
        <Input
          aria-label="Search catalogue"
          className="sm:w-72"
          leadingIcon={<Search />}
          onChange={(event) => {
            setPage(1);
            setSearch(event.target.value);
          }}
          placeholder="Search title, author, ISBN"
          value={search}
        />
        <Select
          aria-label="Title status"
          className="sm:w-44"
          onChange={(event) => {
            setPage(1);
            setStatus(event.target.value);
          }}
          value={status}
        >
          <option value="all">All statuses</option>
          <option value="active">Active</option>
          <option value="retired">Retired</option>
        </Select>
        {!loading && titles.length > 0 ? (
          <TableControlsPagination
            onNext={() => setPage((value) => Math.min(totalPages, value + 1))}
            onPrevious={() => setPage((value) => Math.max(1, value - 1))}
            page={page}
            totalPages={totalPages}
          />
        ) : null}
      </TableControlsBar>
      <TableWrap>
        {loading ? (
          <TableLoading columns={5} label="Loading catalogue…" />
        ) : error ? (
          <TableError description={error} onRetry={() => void load()} />
        ) : titles.length === 0 ? (
          <TableEmpty
            description={
              filtered
                ? "Change the current filters."
                : canCreate
                  ? "Add the first title to the catalogue."
                  : "No titles are available."
            }
            icon={<BookOpen />}
            title={filtered ? "No titles match" : "Catalogue is empty"}
          />
        ) : (
          <TableScroll>
            <Table className="min-w-[760px]">
              <THead>
                <tr>
                  <TH>Title</TH>
                  <TH>Author</TH>
                  <TH>Subject</TH>
                  <TH>Copies</TH>
                  <TH>Status</TH>
                </tr>
              </THead>
              <TBody>
                {titles.map((title) => (
                  <TR
                    className="cursor-pointer"
                    key={title.id}
                    onClick={() => {
                      setSelectedId(title.id);
                      setDrawerOpen(true);
                    }}
                  >
                    <TD>
                      <button
                        className="text-left font-medium text-[var(--text-strong)] hover:text-[var(--brand-strong)]"
                        type="button"
                      >
                        {title.title}
                      </button>
                      <p className="mt-1 text-xs text-[var(--text-muted)]">
                        {title.isbn || "No ISBN"}
                      </p>
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {title.authors.join(", ")}
                    </TD>
                    <TD className="text-[var(--text-muted)]">
                      {title.subject || "—"}
                    </TD>
                    <TD className="font-tabular text-[var(--text-muted)]">
                      {title.available_copy_count} / {title.copy_count}{" "}
                      available
                    </TD>
                    <TD>
                      <Badge tone={statusTone(title.status)}>
                        {displayValue(title.status)}
                      </Badge>
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </TableScroll>
        )}
      </TableWrap>
      <TitleDrawer
        canCreate={canCreate}
        canEdit={canEdit}
        canRetire={canRetire}
        currencies={references?.currencies ?? []}
        onClose={() => setDrawerOpen(false)}
        onSaved={() => {
          setDrawerOpen(false);
          void load();
        }}
        open={drawerOpen}
        titleId={selectedId}
      />
    </div>
  );
}

function TitleDrawer({
  canCreate,
  canEdit,
  canRetire,
  currencies,
  onClose,
  onSaved,
  open,
  titleId,
}: {
  canCreate: boolean;
  canEdit: boolean;
  canRetire: boolean;
  currencies: CurrencyReference[];
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  titleId: string | null;
}) {
  const [record, setRecord] = useState<TitleDetail | null>(null);
  const [copies, setCopies] = useState<CopyRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [addingCopy, setAddingCopy] = useState(false);
  const [title, setTitle] = useState("");
  const [subtitle, setSubtitle] = useState("");
  const [authors, setAuthors] = useState("");
  const [isbn, setIsbn] = useState("");
  const [publisher, setPublisher] = useState("");
  const [year, setYear] = useState("");
  const [edition, setEdition] = useState("");
  const [language, setLanguage] = useState("eng");
  const [subject, setSubject] = useState("");
  const [cost, setCost] = useState("");
  const [currencyId, setCurrencyId] = useState("");
  const [barcode, setBarcode] = useState("");
  const [location, setLocation] = useState("");
  const [condition, setCondition] = useState<CopyCondition>("good");
  const editable = titleId ? canEdit && record?.status === "active" : canCreate;
  const selectedCurrency = useMemo(
    () => currencies.find((currency) => currency.id === currencyId),
    [currencies, currencyId],
  );
  const replacementMinorUnits =
    selectedCurrency?.minor_units ??
    record?.replacement_currency_minor_units ??
    2;
  const apply = (value: TitleDetail | null) => {
    setRecord(value);
    setTitle(value?.title ?? "");
    setSubtitle(value?.subtitle ?? "");
    setAuthors(value?.authors.join(", ") ?? "");
    setIsbn(value?.isbn ?? "");
    setPublisher(value?.publisher ?? "");
    setYear(value?.publication_year?.toString() ?? "");
    setEdition(value?.edition ?? "");
    setLanguage(value?.language_code ?? "eng");
    setSubject(value?.subject ?? "");
    setCost(
      value?.replacement_cost_minor == null
        ? ""
        : String(
            value.replacement_cost_minor /
              10 ** (value.replacement_currency_minor_units ?? 2),
          ),
    );
    setCurrencyId(value?.replacement_currency_id ?? "");
  };
  const load = useCallback(async () => {
    if (!titleId) {
      apply(null);
      setCopies([]);
      return;
    }
    setLoading(true);
    try {
      const [titleResponse, copyResponse] = await Promise.all([
        libraryService.title(titleId),
        libraryService.copies(titleId, { per_page: 100 }),
      ]);
      if (!titleResponse.success || !titleResponse.data)
        throw new Error(
          responseMessage(titleResponse, "Title could not be loaded"),
        );
      apply(titleResponse.data);
      setCopies(copyResponse.success ? (copyResponse.data?.copies ?? []) : []);
    } catch (loadError) {
      toast.error(
        loadError instanceof Error
          ? loadError.message
          : "Title could not be loaded",
      );
      onClose();
    } finally {
      setLoading(false);
    }
  }, [onClose, titleId]);
  useEffect(() => {
    if (open) void load();
  }, [load, open]);
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!editable || saving) return;
    const payload: TitlePayload = {
      title: title.trim(),
      subtitle: optional(subtitle),
      authors: authors
        .split(",")
        .map((value) => value.trim())
        .filter(Boolean),
      isbn: optional(isbn),
      publisher: optional(publisher),
      publication_year: year ? Number(year) : null,
      edition: optional(edition),
      language_code: language.trim().toLowerCase(),
      subject: optional(subject),
      replacement_cost_minor:
        cost && currencyId
          ? Math.round(Number(cost) * 10 ** replacementMinorUnits)
          : null,
      replacement_currency_id: cost && currencyId ? currencyId : null,
    };
    setSaving(true);
    try {
      const response = record
        ? await libraryService.updateTitle(record.id, record.version, payload)
        : await libraryService.createTitle(payload);
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Title could not be saved"));
      toast.success(record ? "Title updated" : "Title created");
      onSaved();
    } catch (saveError) {
      toast.error(
        saveError instanceof Error
          ? saveError.message
          : "Title could not be saved",
      );
    } finally {
      setSaving(false);
    }
  };
  const createCopy = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!record || saving) return;
    setSaving(true);
    try {
      const response = await libraryService.createCopy(record.id, {
        barcode: optional(barcode),
        location: optional(location),
        condition,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Copy could not be added"));
      setCopies((current) => [...current, response.data!]);
      setBarcode("");
      setLocation("");
      setAddingCopy(false);
      toast.success(`Copy ${response.data.accession_number} added`);
    } catch (saveError) {
      toast.error(
        saveError instanceof Error
          ? saveError.message
          : "Copy could not be added",
      );
    } finally {
      setSaving(false);
    }
  };
  const retireTitle = async () => {
    if (!record || saving) return;
    setSaving(true);
    try {
      const response = await libraryService.retireTitle(
        record.id,
        record.version,
      );
      if (!response.success)
        throw new Error(
          responseMessage(response, "Title could not be retired"),
        );
      toast.success("Title retired");
      onSaved();
    } catch (saveError) {
      toast.error(
        saveError instanceof Error
          ? saveError.message
          : "Title could not be retired",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <DialogShell onClose={onClose} open={open}>
      <DialogHeader
        onClose={saving ? undefined : onClose}
        title={titleId ? "Catalogue title" : "New catalogue title"}
      />
      {loading ? (
        <DialogBody>
          <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
            <Loader2 className="size-4 animate-spin" />
            Loading title…
          </div>
        </DialogBody>
      ) : (
        <form onSubmit={submit}>
          <DialogBody className="space-y-7">
            <section className="grid gap-5 sm:grid-cols-2">
              <Field label="Title" wide>
                <Input
                  data-autofocus="true"
                  disabled={!editable}
                  onChange={(event) => setTitle(event.target.value)}
                  required
                  value={title}
                />
              </Field>
              <Field label="Subtitle" wide>
                <Input
                  disabled={!editable}
                  onChange={(event) => setSubtitle(event.target.value)}
                  value={subtitle}
                />
              </Field>
              <Field label="Authors (comma separated)" wide>
                <Textarea
                  disabled={!editable}
                  onChange={(event) => setAuthors(event.target.value)}
                  required
                  value={authors}
                />
              </Field>
              <Field label="ISBN">
                <Input
                  disabled={!editable}
                  onChange={(event) => setIsbn(event.target.value)}
                  value={isbn}
                />
              </Field>
              <Field label="Subject">
                <Input
                  disabled={!editable}
                  onChange={(event) => setSubject(event.target.value)}
                  value={subject}
                />
              </Field>
              <Field label="Publisher">
                <Input
                  disabled={!editable}
                  onChange={(event) => setPublisher(event.target.value)}
                  value={publisher}
                />
              </Field>
              <Field label="Publication year">
                <Input
                  disabled={!editable}
                  max="9999"
                  min="1000"
                  onChange={(event) => setYear(event.target.value)}
                  type="number"
                  value={year}
                />
              </Field>
              <Field label="Edition">
                <Input
                  disabled={!editable}
                  onChange={(event) => setEdition(event.target.value)}
                  value={edition}
                />
              </Field>
              <Field label="Language code">
                <Input
                  disabled={!editable}
                  maxLength={3}
                  minLength={3}
                  onChange={(event) => setLanguage(event.target.value)}
                  required
                  value={language}
                />
              </Field>
              <Field
                label={`Replacement cost${selectedCurrency ? ` (${selectedCurrency.code})` : ""}`}
              >
                <Input
                  disabled={!editable}
                  min="0"
                  onChange={(event) => setCost(event.target.value)}
                  step={1 / 10 ** replacementMinorUnits}
                  type="number"
                  value={cost}
                />
              </Field>
              <Field label="Currency">
                <Select
                  disabled={!editable}
                  onChange={(event) => setCurrencyId(event.target.value)}
                  required={Boolean(cost)}
                  value={currencyId}
                >
                  <option value="">No replacement cost</option>
                  {currencies.map((currency) => (
                    <option key={currency.id} value={currency.id}>
                      {currency.code}
                    </option>
                  ))}
                </Select>
              </Field>
            </section>
            {record ? (
              <section>
                <div className="flex items-center justify-between gap-4 border-b border-[var(--border)] pb-3">
                  <div>
                    <h3 className="text-sm font-semibold text-[var(--text-strong)]">
                      Physical copies
                    </h3>
                    <p className="mt-1 text-xs text-[var(--text-muted)]">
                      Accession numbers are assigned automatically.
                    </p>
                  </div>
                  {canCreate && record.status === "active" ? (
                    <Button
                      onClick={() => setAddingCopy((value) => !value)}
                      size="sm"
                      type="button"
                      variant="secondary"
                    >
                      <Plus className="size-4" />
                      Add copy
                    </Button>
                  ) : null}
                </div>
                {addingCopy ? (
                  <div className="mt-4 space-y-4 border border-[var(--border)] bg-[var(--surface-muted)] p-4">
                    <Field label="Barcode">
                      <Input
                        onChange={(event) => setBarcode(event.target.value)}
                        value={barcode}
                      />
                    </Field>
                    <Field label="Location">
                      <Input
                        onChange={(event) => setLocation(event.target.value)}
                        value={location}
                      />
                    </Field>
                    <Field label="Condition">
                      <Select
                        onChange={(event) =>
                          setCondition(event.target.value as CopyCondition)
                        }
                        value={condition}
                      >
                        <option value="new">New</option>
                        <option value="good">Good</option>
                        <option value="worn">Worn</option>
                        <option value="damaged">Damaged</option>
                      </Select>
                    </Field>
                    <div className="flex justify-end">
                      <Button
                        disabled={saving}
                        onClick={(event) => void createCopy(event)}
                        size="sm"
                        type="button"
                      >
                        Add copy
                      </Button>
                    </div>
                  </div>
                ) : null}
                <div className="mt-3 space-y-2">
                  {copies.length === 0 ? (
                    <p className="py-4 text-sm text-[var(--text-muted)]">
                      No copies have been added.
                    </p>
                  ) : (
                    copies.map((copy) => (
                      <CopyRow
                        canEdit={canEdit}
                        canRetire={canRetire}
                        copy={copy}
                        key={copy.id}
                        onChanged={(value) =>
                          setCopies((current) =>
                            current.map((item) =>
                              item.id === value.id ? value : item,
                            ),
                          )
                        }
                      />
                    ))
                  )}
                </div>
              </section>
            ) : null}
          </DialogBody>
          <DialogFooter>
            {record && canRetire && record.status === "active" ? (
              <Button
                disabled={saving}
                onClick={() => void retireTitle()}
                type="button"
                variant="destructive"
              >
                Retire title
              </Button>
            ) : null}
            <Button
              disabled={saving}
              onClick={onClose}
              type="button"
              variant="secondary"
            >
              Close
            </Button>
            {editable ? (
              <Button
                disabled={saving || !title.trim() || !authors.trim()}
                type="submit"
              >
                {saving ? (
                  <>
                    <Loader2 className="size-4 animate-spin" />
                    Saving…
                  </>
                ) : record ? (
                  "Save title"
                ) : (
                  "Create title"
                )}
              </Button>
            ) : null}
          </DialogFooter>
        </form>
      )}
    </DialogShell>
  );
}

function CopyRow({
  canEdit,
  canRetire,
  copy,
  onChanged,
}: {
  canEdit: boolean;
  canRetire: boolean;
  copy: CopyRecord;
  onChanged: (copy: CopyRecord) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [barcode, setBarcode] = useState(copy.barcode ?? "");
  const [location, setLocation] = useState(copy.location ?? "");
  const [condition, setCondition] = useState(copy.condition);
  const [status, setStatus] = useState(copy.status);
  const [saving, setSaving] = useState(false);
  const save = async () => {
    setSaving(true);
    try {
      const response = await libraryService.updateCopy(copy.id, copy.version, {
        barcode: optional(barcode),
        location: optional(location),
        condition,
        status,
      });
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Copy could not be updated"));
      onChanged(response.data);
      setEditing(false);
      toast.success("Copy updated");
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Copy could not be updated",
      );
    } finally {
      setSaving(false);
    }
  };
  const retire = async () => {
    setSaving(true);
    try {
      const response = await libraryService.retireCopy(copy.id, copy.version);
      if (!response.success || !response.data)
        throw new Error(responseMessage(response, "Copy could not be retired"));
      onChanged(response.data);
      toast.success("Copy retired");
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Copy could not be retired",
      );
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="border border-[var(--border)] p-3">
      {editing ? (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Barcode">
            <Input
              onChange={(event) => setBarcode(event.target.value)}
              value={barcode}
            />
          </Field>
          <Field label="Location">
            <Input
              onChange={(event) => setLocation(event.target.value)}
              value={location}
            />
          </Field>
          <Field label="Condition">
            <Select
              onChange={(event) =>
                setCondition(event.target.value as CopyCondition)
              }
              value={condition}
            >
              <option value="new">New</option>
              <option value="good">Good</option>
              <option value="worn">Worn</option>
              <option value="damaged">Damaged</option>
            </Select>
          </Field>
          <Field label="Status">
            <Select
              onChange={(event) => setStatus(event.target.value as CopyStatus)}
              value={status}
            >
              {["available", "repair", "lost", "retired"].map((value) => (
                <option key={value} value={value}>
                  {displayValue(value)}
                </option>
              ))}
            </Select>
          </Field>
          <div className="flex gap-2 sm:col-span-2">
            <Button
              disabled={saving}
              onClick={() => void save()}
              size="sm"
              type="button"
            >
              Save
            </Button>
            <Button
              onClick={() => setEditing(false)}
              size="sm"
              type="button"
              variant="secondary"
            >
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-3">
          <div className="min-w-0 flex-1">
            <p className="font-tabular text-sm font-semibold text-[var(--text-strong)]">
              {copy.accession_number}
            </p>
            <p className="mt-1 text-xs text-[var(--text-muted)]">
              {copy.location || "No location"} · {displayValue(copy.condition)}
            </p>
          </div>
          <Badge tone={statusTone(copy.status)}>
            {displayValue(copy.status)}
          </Badge>
          {canEdit &&
          !["on_loan", "reserved", "retired"].includes(copy.status) ? (
            <Button
              onClick={() => setEditing(true)}
              size="sm"
              type="button"
              variant="secondary"
            >
              Edit
            </Button>
          ) : null}
          {canRetire && copy.status === "available" ? (
            <Button
              disabled={saving}
              onClick={() => void retire()}
              size="sm"
              type="button"
              variant="ghost"
            >
              Retire
            </Button>
          ) : null}
        </div>
      )}
    </div>
  );
}

function Field({
  children,
  label,
  wide = false,
}: {
  children: ReactNode;
  label: string;
  wide?: boolean;
}) {
  return (
    <div className={wide ? "sm:col-span-2" : ""}>
      <Label>{label}</Label>
      <div className="mt-1.5">{children}</div>
    </div>
  );
}
function allowed(permissions: string[], permission: string) {
  return permissions.includes("*") || permissions.includes(permission);
}
