package dev.yog.script;

/** {@code System.Error}: a failure's description, what {@code Err} carries by default. */
public class Error {
    public final String Message;

    public Error(String message) {
        this.Message = message;
    }

    @Override
    public String toString() {
        return Message;
    }
}
