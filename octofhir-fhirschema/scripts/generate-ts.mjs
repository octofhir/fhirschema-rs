import { CanonicalManager } from '@atomic-ehr/fhir-canonical-manager';
import { translate } from '@atomic-ehr/fhirschema';
import { writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const projectDir = process.argv[2];
const outputDir = process.argv[3];
mkdirSync(outputDir, { recursive: true });

console.log('Initializing FHIR package manager...');
const manager = CanonicalManager({
    packages: ["hl7.fhir.r4b.core"],
    workingDir: join(projectDir, ".fhir")
});

await manager.init();

console.log('Searching for R4B StructureDefinitions...');
// NOTE: package filter doesn't work in @atomic-ehr/fhir-canonical-manager@0.0.15
// so we search all and filter in JavaScript
const entries = await manager.searchEntries({});

// Filter for StructureDefinitions from hl7.fhir.r4b.core package
const allResources = entries.filter(entry => {
    return entry.resourceType === "StructureDefinition" &&  // Only StructureDefinitions
           entry.url &&                                       // Must have URL
           entry.package?.name === "hl7.fhir.r4b.core";       // Only R4B core
});

console.log(`Found ${allResources.length} resources (base + profiles)`);

let converted = 0;
let failed = 0;

for (const entry of allResources) {
    try {
        // Get full StructureDefinition
        const sd = await manager.resolve(entry.url);

        if (!sd) {
            console.error(`  Failed to resolve: ${entry.url}`);
            failed++;
            continue;
        }

        // Convert to schema via translate() - same for all
        const schema = translate(sd);

        // Use unique name from URL to avoid collisions
        // For base: Patient.json
        // For profiles: vitalsigns.json, bp.json, etc.
        const filename = entry.url.split('/').pop();
        writeFileSync(
            join(outputDir, `${filename}.json`),
            JSON.stringify(schema, null, 2)
        );

        converted++;
        if (converted % 50 === 0) {
            console.log(`  Converted ${converted} resources...`);
        }
    } catch (error) {
        console.error(`  Error converting ${entry.url}:`, error.message);
        failed++;
    }
}

console.log(`✅ Conversion complete: ${converted} succeeded, ${failed} failed`);
