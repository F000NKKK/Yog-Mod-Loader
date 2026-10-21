package dev.yog;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * A method the bus calls to answer a service ({@code YogServices}): its
 * parameters are the service request's components, spread as the
 * generated code passes them (yog-event's service definition says them).
 */
@Documented
@Retention(RetentionPolicy.SOURCE)
@Target(ElementType.METHOD)
public @interface YogServiceHost {
}
