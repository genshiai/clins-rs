package jp.genshi.clins
import kotlin.test.*
import java.io.File
class ValidatorTest {
    @Test fun validateR4() {
        Validator(null).use { validator ->
            assertContains(validator.validateJson("""{"resourceType":"Patient","active":true}""", null), "\"status\":\"valid\"")
            assertContains(validator.validateJson("""{"resourceType":"Patient","active":"yes"}""", null), "\"status\":\"invalid\"")
            assertContains(validator.validateJson("""{"resourceType":"Patient"}""", "https://example.org/missing"), "\"status\":\"incomplete\"")
            assertContains(validator.validateJson("""{"resourceType":"Patient","telecom":[{"rank":0}]}""", null), "\"status\":\"invalid\"")
            assertFailsWith<ClinsException> { validator.validateJson("{", null) }
            assertFailsWith<ClinsException> { validator.validateJson("""{"resourceType":"Patient","active":true,"active":false}""", null) }
        }
    }
    @Test fun validateClins() {
        val root = File(System.getProperty("clins.repoRoot"))
        val bundle = File(root, "data/definitions.json")
        assertTrue(bundle.exists(), "Run clins export-definitions before the SDK tests")
        Validator(bundle.readText()).use { validator ->
            for (name in listOf("patient", "conditions", "allergies", "laboratory", "referral", "discharge", "patient_summary", "medications")) {
                val resource = File(root, "crates/core/tests/fixtures/$name.json").readText()
                assertContains(validator.validateJson(resource, null), "\"status\":\"valid\"")
            }
            val patient = File(root, "crates/core/tests/fixtures/patient.json").readText()
            assertContains(validator.validateJson(patient, null), "\"status\":\"valid\"")
            assertContains(validator.validateJson(patient.replace(Regex("\"gender\"\\s*:\\s*\"[^\"]+\""), "\"gender\":\"invalid\""), null), "\"status\":\"invalid\"")
        }
    }
}
