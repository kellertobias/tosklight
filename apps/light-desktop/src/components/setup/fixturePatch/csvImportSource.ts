import { useMemo, useState } from "react";
import {
	type CsvImportField,
	type CsvPositionUnit,
	guessColumnAssignments,
	type ParsedCsv,
	parseCsv,
} from "./csvImport";

export type CsvFirstRow = "header" | "data";

/**
 * The chosen CSV file, its parsed rows, and the operator's column assignments. `onSourceChanged`
 * runs whenever a change invalidates fixture-type selections made for the previous columns.
 */
export function useCsvImportSource(onSourceChanged: () => void) {
	const [fileName, setFileName] = useState("");
	const [parsed, setParsed] = useState<ParsedCsv | null>(null);
	const [fileError, setFileError] = useState("");
	const [firstRow, setFirstRowState] = useState<CsvFirstRow>("header");
	const [assignments, setAssignments] = useState<Array<CsvImportField | null>>(
		[],
	);
	const [positionUnit, setPositionUnit] = useState<CsvPositionUnit>("m");

	const columnCount = parsed
		? Math.max(0, ...parsed.rows.map((row) => row.length))
		: 0;
	const headers = useMemo(
		() =>
			Array.from({ length: columnCount }, (_, index) =>
				firstRow === "header" && parsed?.rows[0]?.[index]
					? parsed.rows[0][index]
					: `Column ${index + 1}`,
			),
		[columnCount, firstRow, parsed],
	);
	const dataRows = useMemo(
		() => (parsed ? parsed.rows.slice(firstRow === "header" ? 1 : 0) : []),
		[firstRow, parsed],
	);
	const rowNumbers = useMemo(
		() => dataRows.map((_, index) => index + (firstRow === "header" ? 2 : 1)),
		[dataRows, firstRow],
	);

	const selectFile = async (file?: File) => {
		onSourceChanged();
		setFileError("");
		setFileName(file?.name ?? "");
		if (!file) {
			setParsed(null);
			setAssignments([]);
			return;
		}
		try {
			const next = parseCsv(new TextDecoder().decode(await file.arrayBuffer()));
			if (!next.rows.length) {
				setParsed(null);
				setFileError(`${file.name} contains no rows.`);
				return;
			}
			const width = Math.max(...next.rows.map((row) => row.length));
			const guessed = guessColumnAssignments(next.rows[0]);
			setParsed(next);
			setFirstRowState(guessed.some(Boolean) ? "header" : "data");
			setAssignments(
				Array.from({ length: width }, (_, index) => guessed[index] ?? null),
			);
		} catch (error) {
			setParsed(null);
			setFileError(
				`${file.name} could not be read: ${error instanceof Error ? error.message : String(error)}`,
			);
		}
	};

	/** Assigns a field to one column; a field already assigned elsewhere moves here. */
	const assign = (column: number, field: CsvImportField | null) => {
		onSourceChanged();
		setAssignments((current) =>
			current.map((existing, index) =>
				index === column ? field : existing === field ? null : existing,
			),
		);
	};

	const setFirstRow = (value: CsvFirstRow) => {
		onSourceChanged();
		setFirstRowState(value);
	};

	return {
		fileName,
		parsed,
		fileError,
		firstRow,
		setFirstRow,
		assignments,
		assign,
		positionUnit,
		setPositionUnit,
		headers,
		dataRows,
		rowNumbers,
		selectFile,
	};
}
