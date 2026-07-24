import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	RestartMode,
	Show,
} from "../../apps/control-ui/e2e/bench/showScenario";

scenario(
	"BENCH-SHOW-004",
	"runs named create, autosave, Save As, revision, and reopen through visible operator controls",
	async (t) => {
		await t.app.open();
		const empty = await t.show.create(`Operator Empty ${crypto.randomUUID()}`);
		await t.show.expect.active(empty);
		await t.show.expect.dirty(false);
		await t.show.save();

		await t.show.use(Show.TwelveDimmers);
		const source = await t.show.saveAs(
			`Portable Source ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(source);
		const revision = await t.show.saveRevision("Approved operator state");
		await t.show.expect.revision({
			number: revision,
			name: "Approved operator state",
		});
		const laterCopy = await t.show.saveAs(
			`Portable Later Copy ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(laterCopy);
		await t.show.load(source);
		await t.show.expect.active(source);
		const revisionCopy = await t.show.loadRevision(source, revision);
		await t.show.expect.active(revisionCopy);
		await t.show.save();
	},
);

scenario(
	"BENCH-SHOW-005",
	"normalizes independent typed API show workflows without inventing a manual save route",
	async (t) => {
		const source = await t.show.via.api.create(
			`API Operator Source ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(source);
		const revision = await t.show.via.api.saveRevision("API checkpoint");
		await t.show.expect.revision({ number: revision, name: "API checkpoint" });
		const copy = await t.show.via.api.saveAs(
			`API Operator Copy ${crypto.randomUUID()}`,
		);
		await t.show.expect.active(copy);
		await t.show.via.api.load(source);
		const revisionCopy = await t.show.via.api.loadRevision(source, revision);
		await t.show.expect.active(revisionCopy);
		const cleanDefault = await t.show.via.api.loadCleanDefault();
		await t.show.expect.active(cleanDefault);
		await t.show.expect.dirty(false);
	},
);

scenario(
	"BENCH-SHOW-006",
	"keeps restart and malformed-show recovery inside the isolated bench",
	async (t) => {
		await t.show.use(Show.CompactRig);
		await t.app.open();
		await t.show.restart(RestartMode.Graceful);
		await t.show.expect.active(Show.CompactRig);
		await t.show.restart(RestartMode.Abrupt);
		await t.show.expect.active(Show.CompactRig);

		await t.show.recovery.prepareMalformedActive();
		await t.show.expect.recoveryRequired();
		const recovered = await t.show.loadCleanDefault();
		await t.show.expect.active(recovered);
		await t.show.expect.recovered();
	},
);
