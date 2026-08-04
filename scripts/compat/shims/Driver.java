// Exercises clients/java/Ttid.java against the binary in TTID_BIN.
// The client returns raw response lines; the harness masks durationMs.
public final class Driver {
    private static final String FIXED = "4SQ1NZT5HC0";
    private static final String UPDATED = "4SQ1NZT5HC0-4SQ1NZT5P1S";
    private static final String DELETED = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK";

    public static void main(String[] args) throws Exception {
        try (Ttid t = new Ttid(System.getenv("TTID_BIN"))) {
            System.out.println("generate=" + t.generate());
            System.out.println("update=" + t.generate(FIXED));
            System.out.println("delete=" + t.generate(UPDATED, true));
            System.out.println("decode=" + t.decodeTime(DELETED));
            System.out.println("isTTID=" + t.isTTID(FIXED));
            System.out.println("isTTID-bad=" + t.isTTID("nope"));
            System.out.println("isUUID=" + t.isUUID("3f2504e0-4f89-41d3-9a0c-0305e82c3301"));
            System.out.println("isUUID-bad=" + t.isUUID("nope"));
            try {
                t.generate(DELETED);
                System.out.println("error=NO ERROR RAISED");
            } catch (Exception e) {
                System.out.println("error=" + e.getMessage());
            }
        }
    }
}
